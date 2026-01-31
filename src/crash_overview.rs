use quick_xml::{Reader, escape::unescape, events::Event};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::crash_endpoint::File;

#[derive(Serialize, Deserialize)]
pub struct CrashOverview {
    pub error: String,
    pub callstack: String,
    pub user_description: String,
    pub files: Vec<String>,
}
impl CrashOverview {
    pub fn parse(crash_context_xml: &str, files: &Vec<File>) -> Self {
        let mut reader = Reader::from_str(crash_context_xml);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        let mut tag_stack: Vec<String> = Vec::new();

        let mut error_message: Option<String> = None;
        let mut callstack: Option<String> = None;
        let mut user_description: Option<String> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                    tag_stack.push(tag);
                }
                Ok(Event::End(_)) => {
                    tag_stack.pop();
                }
                Ok(Event::Text(e)) => {
                    let raw = String::from_utf8_lossy(e.as_ref());
                    let text = unescape(&raw).unwrap().into_owned();

                    if tag_stack == ["FGenericCrashContext", "RuntimeProperties", "ErrorMessage"] {
                        error_message = Some(error_message.take().unwrap_or_default() + &text)
                    } else if tag_stack
                        == ["FGenericCrashContext", "RuntimeProperties", "CallStack"]
                    {
                        callstack = Some(callstack.take().unwrap_or_default() + &text)
                    } else if tag_stack
                        == [
                            "FGenericCrashContext",
                            "RuntimeProperties",
                            "UserDescription",
                        ]
                    {
                        user_description = Some(user_description.take().unwrap_or_default() + &text)
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => error!("XML error at postition {}: {}", reader.buffer_position(), e),
                _ => {}
            }
            buf.clear();
        }

        Self {
            error: error_message.unwrap_or_default(),
            callstack: callstack.unwrap_or_default(),
            user_description: user_description.unwrap_or_default(),
            files: files.iter().map(|file| file.name.to_string()).collect(),
        }
    }
}
