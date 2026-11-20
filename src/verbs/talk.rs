//! `bmo talk`: words for a room, as a line or as a spoken line in a voice (docs/boundary-api.md, 「The unit
//! command」). Neither is said here: words go into a room only as a round's message, which the round writes and
//! its instruction stages check, so the receiver refuses them in the asker's own round for that round's room and hands them
//! to the room's supervisor everywhere else (docs/messages.md, 「Words handed to the room's supervisor」). A spoken
//! line is asked by its words — the voice, the written words and the spoken words where given — and nothing is
//! synthesised.

use room_calls::sends::{Act, SayInput, TalkInput};

use crate::command::{SayArgs, words};
use crate::config::Config;
use crate::verbs::{VerbError, asked_sends};

/// `talk`: the words asked as a line, or, where `voice` is named, as a spoken line with `spoken` as the words the
/// voice would say in their place.
pub async fn talk(
  config: &Config,
  via: Option<String>,
  args: SayArgs,
  voice: Option<String>,
  spoken: Option<String>,
) -> Result<Vec<String>, VerbError> {
  let chat = args.chat.unwrap_or(config.owner);
  let asker = asked_sends::shell_asker(chat, via, args.by, "talk");
  let text = words(args.text)?;
  let act = match voice {
    None => Act::Say(SayInput {
      chat: None,
      via: None,
      text,
      reply: args.reply,
      reply_to: args.reply_to,
    }),
    Some(voice) => Act::Talk(TalkInput {
      chat: None,
      via: None,
      voice,
      text,
      spoken,
      reply_to: args.reply_to,
    }),
  };

  asked_sends::ask(&config.data_root, asker, act).await
}
