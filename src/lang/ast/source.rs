use crate::lang::ast::location::Location;
use crate::lang::ast::tracked_string::TrackedString;
use crate::lang::errors::{CrushResult, error};
use crate::lang::serialization::model::source::Replacement;
use crate::lang::serialization::model::{Element, element, source};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState, model};
use std::fmt::{Debug, Display, Formatter};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum SourceType {
    Input,
    File(PathBuf),
}

/// A Source tracks the start and end of the definition of something in source code. It is used
/// by jobs, closures, commands, etc in order to be able to give good error reporting.
#[derive(Clone, Debug)]
pub struct Source {
    source_type: SourceType,
    string: Arc<str>,
    location: Location,
    replacement: Option<String>,
}

impl Display for SourceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Input => Ok(()),
            SourceType::File(path) => {
                path.fmt(f)?;
                f.write_str(":\n")
            }
        }
    }
}

impl Source {
    pub fn new(source_type: SourceType, string: Arc<str>) -> Source {
        Source {
            source_type,
            location: Location::new(0, string.len()),
            string,
            replacement: None,
        }
    }

    pub fn start_line(&self) -> usize {
        self.string[0..self.location.start].lines().count()
    }

    pub fn substring(&self, location: Location) -> Source {
        Source {
            source_type: self.source_type.clone(),
            string: self.string.clone(),
            location,
            replacement: None,
        }
    }

    pub fn subtrackedstring(&self, ts: &TrackedString) -> Source {
        if ts.string != self.string[ts.location.start..ts.location.end] {
            Source {
                source_type: self.source_type.clone(),
                string: self.string.clone(),
                location: ts.location,
                replacement: Some(ts.string.clone()),
            }
        } else {
            Source {
                source_type: self.source_type.clone(),
                string: self.string.clone(),
                location: ts.location,
                replacement: None,
            }
        }
    }

    pub fn location(&self) -> Location {
        self.location
    }

    pub fn string(&self) -> String {
        match &self.replacement {
            Some(s) => s.clone(),
            None => self.string[self.location.start..self.location.end].to_string(),
        }
    }

    pub fn str(&self) -> &str {
        match &self.replacement {
            Some(s) => s.as_str(),
            None => &self.string[self.location.start..self.location.end],
        }
    }

    pub fn trace(&self, name: &Option<String>) -> String {
        if let Ok((line_number, _previous_line, current_line)) = self.show_internal() {
            let name = name.as_ref().map(|s| s.as_str()).unwrap_or(&"<anonymous>");
            let mut res = match &self.source_type {
                SourceType::Input => format!("<input> ({}):", name),
                SourceType::File(file) => {
                    format!("File {}, line {} ({}):", file.display(), line_number, name)
                }
            };

            res.push('\n');
            res.push_str(current_line.as_str());
            res
        } else {
            "<error>".to_string()
        }
    }

    pub fn show(&self) -> CrushResult<String> {
        let (line_number, previous_line, current_line) = self.show_internal()?;

        match &self.source_type {
            SourceType::Input => Ok(current_line),
            SourceType::File(file) => match previous_line {
                None => Ok(format!(
                    "{}:\n{} {}",
                    file.display(),
                    line_number,
                    current_line
                )),
                Some(previous) => Ok(format!(
                    "{}:\n{:<3} {}\n{:<3} {}",
                    file.display(),
                    line_number - 1,
                    previous,
                    line_number,
                    current_line
                )),
            },
        }
    }

    pub fn show_internal(&self) -> CrushResult<(usize, Option<String>, String)> {
        let mut previous_line = None;
        let mut current_line = Vec::new();
        let mut is_highlight = false;
        let mut line = 1;

        for (idx, chr) in self.string.bytes().enumerate() {
            match (
                idx >= self.location.start,
                idx >= self.location.end,
                chr as char,
            ) {
                (_, true, '\n') => break,
                (false, _, '\n') => {
                    line += 1;
                    previous_line = Some(current_line.clone());
                    current_line.clear()
                }
                (_, _, _) => {
                    if idx == self.location.start {
                        current_line.append(&mut "\x1b[31m".as_bytes().to_vec());
                        is_highlight = true;
                    }
                    current_line.push(chr);
                    if idx == self.location.end {
                        current_line.append(&mut "\x1b[0m".as_bytes().to_vec());
                        is_highlight = false;
                    }
                }
            }
        }
        if is_highlight {
            current_line.append(&mut "\x1b[0m".as_bytes().to_vec());
        }
        match previous_line {
            None => Ok((line, None, String::try_from(current_line)?)),
            Some(p) => Ok((
                line,
                Some(String::try_from(p)?),
                String::try_from(current_line)?,
            )),
        }
    }
}

impl Serializable<Source> for Source {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<Source> {
        match elements[id]
            .element
            .as_ref()
            .ok_or(format!("Invalid index {} while deserializing source", id))?
        {
            element::Element::Source(s) => {
                let replacement = match s.replacement.as_ref() {
                    None => None,
                    Some(Replacement::HasReplacement(_)) => None,
                    Some(Replacement::ReplacementValue(idx)) => {
                        Some(String::deserialize(*idx as usize, elements, state)?)
                    }
                };
                let source_type = match s.source_type.as_ref() {
                    None => SourceType::Input,
                    Some(source::SourceType::Input(_)) => SourceType::Input,
                    Some(source::SourceType::File(idx)) => {
                        SourceType::File(PathBuf::deserialize(*idx as usize, elements, state)?)
                    }
                };
                Ok(Source {
                    string: Arc::deserialize(s.string as usize, elements, state)?,
                    location: Location::deserialize(s.location as usize, elements, state)?,
                    replacement,
                    source_type,
                })
            }
            _ => error(format!(
                "Expected a source, got something else on index {}",
                id
            )),
        }
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let string = self.string.serialize(elements, state)? as u64;
        let location = self.location.serialize(elements, state)? as u64;
        let replacement = Some(match self.replacement.as_ref() {
            None => Replacement::HasReplacement(false),
            Some(replacement) => {
                Replacement::ReplacementValue(replacement.serialize(elements, state)? as u64)
            }
        });

        let source_type = Some(match &self.source_type {
            SourceType::Input => source::SourceType::Input(true),
            SourceType::File(f) => source::SourceType::File(f.serialize(elements, state)? as u64),
        });

        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::Source(model::Source {
                string,
                location,
                replacement,
                source_type,
            })),
        });
        Ok(idx)
    }
}
