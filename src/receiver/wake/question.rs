//! The owner answering a question a job put to him, taken before anybody is woken (docs/bmo-window.md): his
//! private chat holds an open question, and what he typed names exactly one of its options, or is talk over it
//! where its asker takes talk. Recording either is not answering him, so it costs no run and happens during a
//! rest too: a job is standing still waiting for it. The questions themselves are bmo-job's rows, reached
//! through its port.

use receiver_policy::commands::COMMANDS;
use receiver_ports::http::PortFailure;
use receiver_ports::job::{Chosen, Talked};

use crate::receiver::{NotReceived, Receiver};

// A question that is gone or settled already answers the talk with a refusal of its own.
const NOT_TALKABLE: [&str; 2] = ["question_absent", "question_takes_no_talk"];

/// Whether the owner's words answered the question open in his chat, or were written onto it as talk. The
/// option is read from the row's words; the talk is the words he typed.
pub async fn answered(
  receiver: &Receiver,
  chat_id: i64,
  text: &str,
  said: &str,
) -> Result<bool, NotReceived> {
  // Where a chat somehow holds two, the one asked last is the one being waited on.
  let open = receiver.units.job.open_questions(chat_id).await.map_err(NotReceived::from)?;
  let Some(question) = open.first() else {
    return Ok(false);
  };

  match chosen_option(text, &question.options) {
    Some(chosen) => {
      let answer = Chosen {
        id: question.id,
        chosen,
      };

      receiver.units.job.settle(&answer).await.map_err(NotReceived::from)?;
    }
    // A file with nothing written beside it says nothing a job could answer.
    None if question.talks && !said.trim().is_empty() => {
      let talked = Talked {
        id: question.id,
        say: said.to_owned(),
      };

      match receiver.units.job.talk(&talked).await {
        Ok(()) => {}
        Err(why) if refused(&why) => return Ok(false),
        Err(why) => return Err(NotReceived::from(why)),
      }
    }
    None => return Ok(false),
  }

  Ok(true)
}

// Whether a talk came back as the question's own refusal, rather than as the unit not answering.
fn refused(why: &PortFailure) -> bool {
  why.code().is_some_and(|code| NOT_TALKABLE.contains(&code))
}

// Text as an option is matched against it: its edges trimmed, lower case, every run of space one.
fn flatten(text: &str) -> String {
  text.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Which option an owner's message names, counted from zero, or nothing for a message that is not an answer
/// at all. The match is narrow, because everything it takes is something his session never sees: a bare number
/// that is one of the options, as it stands or with a suffix the policy names after it, or a piece of text that
/// stands in exactly one option. A message of several lines is never an answer.
pub fn chosen_option(text: &str, options: &[String]) -> Option<i64> {
  let said = text.trim();

  if said.is_empty() || said.contains('\n') {
    return None;
  }

  let bare = COMMANDS
    .option_suffixes
    .iter()
    .find_map(|suffix| said.strip_suffix(suffix).map(str::trim_end))
    .unwrap_or(said);

  if let Ok(number) = bare.parse::<i64>()
    && number.to_string() == bare
  {
    let count = i64::try_from(options.len()).unwrap_or(i64::MAX);

    return (1..=count).contains(&number).then_some(number - 1);
  }

  let wanted = flatten(said);
  let mut named = options.iter().enumerate().filter(|(_, option)| flatten(option).contains(&wanted));

  match (named.next(), named.next()) {
    (Some((at, _)), None) => i64::try_from(at).ok(),
    _ => None,
  }
}
