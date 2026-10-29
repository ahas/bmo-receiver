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
use bmo_receiver::command::{
  self, ConsultArgs, DrawWord, PhotoArgs, PinArgs, ReactArgs, SayArgs, StickerArgs, TalkArgs,
};
use bmo_receiver::config::{self, Config};
use bmo_receiver::receiver::handover::routes as handover_routes;
use bmo_receiver::routes::{routes, served};
use bmo_receiver::service_start::{Standing, keep, settle};
use bmo_receiver::store::migration::migrate;
use bmo_receiver::tool_env::{Written, base_address, write_env};
use bmo_receiver::verbs::VerbError;
use clap::{Parser, Subcommand};
use receiver_policy::tool_host::TOOL_HOST;
use receiver_ports::tool_host::HttpToolHost;
use reqwest::Client;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::time::timeout;

/// How long the going-down line may hold the stop.
const STOP_LINE_WAIT: Duration = Duration::from_secs(5);

/// How long the service is waited on for one health answer.
const PATIENCE: Duration = Duration::from_secs(5);

#[derive(Parser)]
#[command(
  name = "bmo-receiver",
  version,
  about = "The chat receiver: rooms, their rules and rights, room windows and the wake."
)]
struct Cli {
  /// The configuration file; the release's own where none is given.
  #[arg(long)]
  config: Option<PathBuf>,
  /// The road the caller's turn is read on, which a send goes out on where it answers in the room.
  #[arg(long, global = true)]
  via: Option<String>,
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
  /// The nouns `bmo` dispatches to this command, one a line.
  Nouns,
  /// Whether work stands under way that a restart would cut: 0 where none does, 1 where some does or the agency
  /// did not answer, each one named.
  Quiet {
    /// Counts every open run of a job too, for a unit whose restart can cut a worker's tool calls mid-turn.
    #[arg(long)]
    with_jobs: bool,
  },
  /// The owner's private room stood up once on the record alone, its number printed for the configuration's
  /// `owner_room`; run again, the one stood up before.
  OwnerRoom {
    /// The room's title.
    #[arg(long, value_name = "words")]
    title: String,
  },
  /// Moves the old archive's exported rows into this store (docs/store.md, TODO #1923).
  Import {
    /// The export directory the item's export hand wrote.
    #[arg(long, value_name = "dir")]
    export: PathBuf,
    /// The PostgreSQL database written.
    #[arg(long, value_name = "url")]
    database: String,
    /// The prefix the old archive's paths carry in the two path columns, rewritten to the data root.
    #[arg(long, value_name = "prefix")]
    stale_root: String,
    /// The data root those files stand under here, whose relative layout the old paths keep.
    #[arg(long, value_name = "dir")]
    data_root: PathBuf,
    /// Reads, checks and counts everything, and writes nothing.
    #[arg(long)]
    dry_run: bool,
  },
  /// A line said in a room, through the running receiver.
  Room {
    #[command(subcommand)]
    word: RoomWord,
  },
  /// A line into a room, as text or spoken in a voice, through the running receiver: the mouth group's
  /// first-class verb `bmo talk`.
  Talk(TalkArgs),
  /// A picture, or several as one album, through the running receiver.
  Photo(PhotoArgs),
  /// An emoji reaction on a message, through the running receiver.
  React(ReactArgs),
  /// A message pinned, or unpinned, through the running receiver.
  Pin(PinArgs),
  /// One of the deployment's stickers, or the set listed, through the running receiver.
  Sticker(StickerArgs),
  /// The consult room read or set, the block an agent is shown, or a line said there.
  Consult(ConsultArgs),
  /// The fair draw made in a room or checked.
  Draw {
    #[command(subcommand)]
    word: DrawWord,
  },
}

#[derive(Subcommand)]
enum RoomWord {
  /// A line said in a room, without waiting for it.
  Say(SayArgs),
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
  ready: bool,
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

  // What only tells another unit this one stands — the cache warmer's targets and the agency's hook
  // points — is offered beside the serve rather than before it: a unit down when the receiver starts
  // costs its own patience, never the rooms' first answer.
  let _settling = {
    let receiver = standing.receiver.clone();
    let consumer = config.consumer.clone();
    let listen = config.listen.clone();
    let token = config.token.clone();

    tokio::spawn(async move { settle(&receiver, &consumer, &listen, &token).await })
  };

  keep(&standing);

  // The tool host's catalogue is heard where a control socket can be named (docs/boundary-api.md, 「The tool
  // host」).
  if let Some((socket, token_file)) = config.tool_host() {
    let host = HttpToolHost::new(socket, token_file, Duration::from_secs(TOOL_HOST.silent_seconds));

    bmo_receiver::tools_heard::keep(standing.receiver.clone(), Arc::new(host));
  }

  let listener = TcpListener::bind(&config.listen).await;
  let listener = listener.map_err(|why| format!("{}: {why}", config.listen))?;
  let bound = listener.local_addr().map_err(|why| why.to_string())?;

  println!("bmo-receiver: answering on {bound}");
  let _ = io::stdout().flush();

  let app = routes(standing.receiver.clone(), config.token.as_str())
    .merge(handover_routes(standing.receiver.clone(), config.token.as_str()).reset_fallback());

  // The streams are told as the signal comes, not after the serve lets go: a stream stands open until it is
  // told, and the serve waits on every one of them before it returns.
  let news = standing.news.clone();
  let told = async move {
    stopped().await;
    news.stopping().await;
  };
  let served = axum::serve(listener, app)
    .with_graceful_shutdown(told)
    .await
    .map_err(|why| why.to_string());

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

/// Whether work stands under way, each one named on its own line.
async fn quiet(config: &Config, jobs: bool) -> u8 {
  let units = config.units();

  match bmo_receiver::quiet::asked(units.agency.as_ref(), bmo_receiver::now_ms(), jobs).await {
    Ok(under_way) if under_way.is_empty() => 0,
    Ok(under_way) => {
      for line in under_way {
        println!("{line}");
      }

      1
    }
    Err(why) => {
      println!("the agency did not answer which runs are open: {why}");

      1
    }
  }
}

/// The owner's private room stood up, or the one stood up before, and its number printed.
async fn owner_room(config: &Config, title: &str) -> Result<u8, String> {
  if title.trim().is_empty() {
    return Err("--title names the room's title".to_owned());
  }

  let db = config.connect().await?;
  let (room, made) = bmo_receiver::owner_room::stand_up(&db, &config.agent, title.trim())
    .await
    .map_err(|why| why.detail.to_owned())?;

  if made {
    println!("{room}");
    eprintln!(
      "bmo-receiver: the owner's private room stood up as {room}; name it owner_room in the configuration"
    );
  } else {
    println!("{room}");
    eprintln!("bmo-receiver: the owner's private room already stands as {room}");
  }

  Ok(0)
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

  health.ready.then_some(()).ok_or_else(|| format!("{} answers not ready", url))
}

/// What a noun's words came to: the lines it said, printed, and how it ended.
fn answered(done: Result<Vec<String>, VerbError>) -> u8 {
  match done {
    Ok(lines) => {
      for line in lines {
        println!("{line}");
      }

      0
    }
    Err(VerbError::Partly { lines, refusal }) => {
      for line in lines {
        println!("{line}");
      }

      if !refusal.is_empty() {
        eprintln!("bmo-receiver: {refusal}");
      }

      1
    }
    Err(why) => {
      eprintln!("bmo-receiver: {why}");

      1
    }
  }
}

#[tokio::main]
async fn main() -> ExitCode {
  let cli = Cli::parse();

  match cli.command {
    Command::Routes => {
      for name in served() {
        println!("{name}");
      }

      return ExitCode::SUCCESS;
    }
    Command::Nouns => {
      for noun in command::NOUNS {
        println!("{noun}");
      }

      return ExitCode::SUCCESS;
    }
    _ => {}
  }

  // The import reads its own arguments and never the deployment's configuration.
  if let Command::Import {
    ref export,
    ref database,
    ref stale_root,
    ref data_root,
    dry_run,
  } = cli.command
  {
    let roots = bmo_receiver::import::Roots::new(stale_root.clone(), data_root.clone());
    let ended = match bmo_receiver::import::run(database, export, dry_run, &roots).await {
      Ok(report) => {
        for moved in &report.tables {
          println!(
            "{} -> {}: {} exported, {} written",
            moved.source, moved.target, moved.exported, moved.written
          );
        }
        println!(
          "{} at snapshot {} read {}: {} rows sampled, {} failures",
          if report.dry_run { "dry run" } else { "import" },
          report.snapshot,
          report.read_at,
          report.sample.checked,
          report.sample.failures.len()
        );
        for failure in &report.sample.failures {
          eprintln!("  {failure}");
        }
        if report.sample.failures.is_empty() { 0 } else { 1 }
      }
      Err(why) => {
        eprintln!("bmo-receiver: {why}");
        1
      }
    };

    return ExitCode::from(ended);
  }

  let config = match Config::read(&config::path(cli.config.as_deref())) {
    Ok(config) => config,
    Err(why) => {
      eprintln!("bmo-receiver: the configuration could not be read: {why}");

      return ExitCode::from(2);
    }
  };

  let ended = match cli.command {
    Command::Serve => serve(&config).await.map(|()| 0),
    Command::DeliverWaiting => deliver_waiting(&config).await,
    Command::OwnerRoom { ref title } => owner_room(&config, title).await,
    Command::Quiet { with_jobs } => Ok(quiet(&config, with_jobs).await),
    Command::Health => ask_health(&config).await.map(|()| 0),
    Command::ToolEnv => write_env(&config).map(|written| {
      match written {
        Written::Written(path) => println!("bmo-receiver: wrote {}", path.display()),
        Written::Unchanged(path) => println!("bmo-receiver: {} already stands as written", path.display()),
      }

      0
    }),
    Command::Room {
      word: RoomWord::Say(args),
    } => Ok(answered(command::room_say(&config, cli.via.clone(), args).await)),
    Command::Talk(args) => Ok(answered(command::talk(&config, cli.via.clone(), args).await)),
    Command::Photo(args) => Ok(answered(command::photo(&config, cli.via.clone(), args).await)),
    Command::React(args) => Ok(answered(command::react(&config, cli.via.clone(), args).await)),
    Command::Pin(args) => Ok(answered(command::pin(&config, cli.via.clone(), args).await)),
    Command::Sticker(args) => Ok(answered(command::sticker(&config, cli.via.clone(), args).await)),
    Command::Consult(args) => Ok(answered(command::consult(&config, args).await)),
    Command::Draw { word } => Ok(answered(command::draw(&config, cli.via.clone(), word).await)),
    Command::Routes | Command::Nouns | Command::Import { .. } => {
      unreachable!("routes, nouns and the import are answered before the configuration is read")
    }
  };

  match ended {
    Ok(code) => ExitCode::from(code),
    Err(why) => refused(why),
  }
}
