//! The receiver service's binary (docs/boundary-api.md, 「Configuration」): `serve` runs the migrations, stands
//! the receiver, settles the start and answers the routes on the configured address until the process is told
//! to stop; `routes` prints every route it answers, one method and path to a line; `deliver-waiting` takes one
//! pass over the instruction rows a job's end left waiting and tells them, as the receiver's sweep does, with
//! no clock of the receiver's: what a check stands in for the sweep with once a runner has ended; `tool-env`
//! writes where the receiver's tools find it (「Its tools」) and ends; `health` ends 0 only where the service the
//! configuration names answers ready (「Health」).

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::instructions::{Rooms, deliver_pending};
use bmo_receiver::chat::restart::say_stop;
use bmo_receiver::config::Config;
use bmo_receiver::receiver::handover::routes as handover_routes;
use bmo_receiver::routes::{routes, served};
use bmo_receiver::service_start::{Standing, keep, settle};
use bmo_receiver::store::migration::migrate;
use bmo_receiver::tool_env::{Written, base_address, write_env};
use clap::{Parser, Subcommand};
use reqwest::Client;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::time::timeout;

/// How long the going-down line may hold the stop.
const STOP_LINE_WAIT: Duration = Duration::from_secs(5);

/// How long the service is waited on for one health answer.
const PATIENCE: Duration = Duration::from_secs(5);

/// The configuration file the service reads, in the release that links to the data root's own.
const SETTINGS: &str = "settings.toml";

#[derive(Parser)]
#[command(
  name = "bmo-receiver",
  version,
  about = "The chat receiver: rooms, their rules and rights, room windows and the wake."
)]
struct Cli {
  /// The configuration file.
  #[arg(long, default_value = SETTINGS)]
  config: PathBuf,
  #[command(subcommand)]
  command: Command,
}

#[derive(Subcommand)]
enum Command {
  /// The migrations run, the receiver stands and the routes answer until the process is told to stop.
  Serve,
  /// Prints every route the receiver answers, one method and path to a line.
  Routes,
  /// One pass over the instruction rows a job's end left waiting, told as the receiver's sweep tells them.
  DeliverWaiting,
  /// The data root's env file written from the configuration: where the receiver's tools find it.
  ToolEnv,
  /// Whether the service the configuration names answers ready: exit code 0 where it does, 1 where it does not.
  Health,
}

fn refused(why: impl std::fmt::Display) -> ExitCode {
  eprintln!("bmo-receiver: {why}");

  ExitCode::FAILURE
}

async fn stopped() {
  let interrupted = tokio::signal::ctrl_c();
  let terminated = async {
    match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
      Ok(mut terminate) => {
        terminate.recv().await;
      }
      Err(_) => std::future::pending::<()>().await,
    }
  };

  tokio::select! {
    _ = interrupted => {}
    () = terminated => {}
  }
}

#[derive(Deserialize)]
struct Up {
  ok: bool,
}

/// The service standing: the migrations run, the receiver stood on its database, the start settled, the clocks
/// kept and the routes answered — the receiver's own and the chat services' hand-overs, whose fallback the
/// receiver's own table carries — until the process is told to stop, when the streams are told first and the
/// going-down line said after.
async fn serve(config: &Config) -> Result<(), String> {
  let db = config.connect().await?;

  let migrated = migrate(&db).await;
  migrated.map_err(|why| format!("the migrations: {why}"))?;

  let standing = Standing::stood(config, db).await?;

  settle(&standing.receiver, &config.consumer, &config.listen, &config.token).await;

  keep(&standing);

  let listener = TcpListener::bind(&config.listen).await;
  let listener = listener.map_err(|why| format!("{}: {why}", config.listen))?;
  let bound = listener.local_addr().map_err(|why| why.to_string())?;

  println!("bmo-receiver: answering on {bound}");
  let _ = io::stdout().flush();

  let app = routes(standing.receiver.clone(), config.token.as_str())
    .merge(handover_routes(standing.receiver.clone(), config.token.as_str()).reset_fallback());

  let served = axum::serve(listener, app)
    .with_graceful_shutdown(stopped())
    .await
    .map_err(|why| why.to_string());

  standing.news.stopping().await;
  let _ = timeout(STOP_LINE_WAIT, say_stop(&standing.receiver)).await;

  served
}

/// One pass over the rows waiting, as the receiver's sweep does them.
async fn deliver_waiting(config: &Config) -> Result<u8, String> {
  let db = config.connect().await?;
  let standing = Standing::stood(config, db).await?;

  let rooms: Arc<dyn Rooms> = Arc::new(standing.runner.clone());
  let mut last = 0;

  deliver_pending(
    &standing.receiver,
    rooms.as_ref(),
    &mut last,
    bmo_receiver::now_ms(),
    receiver_policy::chat_round::CHAT_ROUND.instructions.cold_wake_gap_ms,
  )
  .await
  .map(|_| 0)
  .map_err(|why| why.detail.to_owned())
}

/// Whether the service the configuration names answers ready.
async fn ask_health(config: &Config) -> Result<(), String> {
  let url = format!("{}/health", base_address(&config.listen));
  let client = Client::builder()
    .no_proxy()
    .timeout(PATIENCE)
    .build()
    .map_err(|why| format!("no client for {url}: {why}"))?;
  let answer = client
    .get(&url)
    .bearer_auth(&config.token)
    .send()
    .await
    .map_err(|why| format!("{url} did not answer: {why}"))?;
  let status = answer.status();

  if !status.is_success() {
    return Err(format!("{url} answered {status}"));
  }

  let bytes = answer.bytes().await.map_err(|why| format!("{url} answered unreadably: {why}"))?;
  let health: Up =
    serde_json::from_slice(&bytes).map_err(|why| format!("{url} answered no health: {why}"))?;

  health.ok.then_some(()).ok_or_else(|| format!("{} answers not up", url))
}

#[tokio::main]
async fn main() -> ExitCode {
  let cli = Cli::parse();

  if let Command::Routes = cli.command {
    let names = served();

    for name in names {
      println!("{name}");
    }

    return ExitCode::SUCCESS;
  }

  let config = match Config::read(&cli.config) {
    Ok(config) => config,
    Err(why) => {
      eprintln!("bmo-receiver: the configuration could not be read: {why}");

      return ExitCode::from(2);
    }
  };

  let ended = match cli.command {
    Command::Serve => serve(&config).await.map(|()| 0),
    Command::DeliverWaiting => deliver_waiting(&config).await,
    Command::Health => ask_health(&config).await.map(|()| 0),
    Command::ToolEnv => write_env(&config).map(|written| {
      match written {
        Written::Written(path) => println!("bmo-receiver: wrote {}", path.display()),
        Written::Unchanged(path) => println!("bmo-receiver: {} already stands as written", path.display()),
      }

      0
    }),
    Command::Routes => unreachable!("the routes are printed before the configuration is read"),
  };

  match ended {
    Ok(code) => ExitCode::from(code),
    Err(why) => refused(why),
  }
}
