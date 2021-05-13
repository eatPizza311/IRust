use std::io::{StdinLock, Write};

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use irust_api::{Command, GlobalVariables, Hook, Message, ScriptInfo};

fn main() {
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();

    let message: Message = bincode::deserialize_from(&mut handle).unwrap();
    assert_eq!(message, Message::Greeting);

    if message == Message::Greeting {
        let script_info = ScriptInfo {
            name: "Vim".into(),
            hooks: vec![Hook::InputEvent, Hook::OutputEvent],
            path: std::env::current_exe().unwrap(),
            is_daemon: true,
        };
        bincode::serialize_into(std::io::stdout(), &script_info).unwrap();
        std::io::stdout().flush().unwrap();
    }

    let mut active = true;
    let mut mode = Mode::Insert;
    let mut state = State::Empty;
    loop {
        // message is Message::Hook
        let _message: Message = bincode::deserialize_from(&mut handle).unwrap();
        let hook: Hook = bincode::deserialize_from(&mut handle).unwrap();

        match hook {
            Hook::InputEvent => handle_input_event(&mut handle, &mut state, &mut mode, &active),
            Hook::OutputEvent => {
                handle_output_event(&mut handle, &mut active);
            }

            _ => unreachable!(),
        }
        std::io::stdout().flush().unwrap();
    }

    fn handle_output_event(mut handle: &mut StdinLock, active: &mut bool) {
        let _g: GlobalVariables = bincode::deserialize_from(&mut handle).unwrap();
        let input: String = bincode::deserialize_from(&mut handle).unwrap();
        if input.starts_with(":vim") {
            let action = input.split_whitespace().nth(1);
            match action {
                Some("on") => {
                    *active = true;
                    bincode::serialize_into(
                        std::io::stdout(),
                        &Some("vim mode activated".to_string()),
                    )
                    .unwrap();
                }
                Some("off") => {
                    *active = false;
                    bincode::serialize_into(
                        std::io::stdout(),
                        &Some("vim mode deactivated".to_string()),
                    )
                    .unwrap();
                }
                _ => {
                    bincode::serialize_into(
                        std::io::stdout(),
                        &Some(format!("vim mode state: {}", active)),
                    )
                    .unwrap();
                }
            }
        } else {
            let no_action: Option<String> = None;
            bincode::serialize_into(std::io::stdout(), &no_action).unwrap();
        }
    }

    fn handle_input_event(
        mut handle: &mut StdinLock,
        state: &mut State,
        mode: &mut Mode,
        active: &bool,
    ) {
        let global_variables: GlobalVariables = bincode::deserialize_from(&mut handle).unwrap();
        let event: Event = bincode::deserialize_from(&mut handle).unwrap();
        if !active {
            let cmd: Option<Command> = None;
            bincode::serialize_into(std::io::stdout(), &cmd).unwrap();
            return;
        }

        macro_rules! reset_state {
            () => {
                *state = State::Empty;
            };
        }

        let cmd = (|| match event {
            Event::Key(key) => match key {
                KeyEvent {
                    code: KeyCode::Char(c),
                    modifiers,
                } => {
                    if modifiers != KeyModifiers::NONE && modifiers != KeyModifiers::SHIFT {
                        return None;
                    }

                    if *mode == Mode::Insert {
                        Some(Command::HandleCharacter(c))
                    } else {
                        match *state {
                            State::f => return Some(Command::MoveForwardTillChar(c)),
                            State::F => return Some(Command::MoveBackwardTillChar(c)),
                            State::r => {
                                return Some(Command::Multiple(vec![
                                    Command::HandleDelete,
                                    Command::HandleCharacter(c),
                                    Command::HandleLeft,
                                ]))
                            }
                            State::ci => {
                                *mode = Mode::Insert;
                                return Some(Command::Multiple(vec![
                                    Command::MoveBackwardTillChar(c),
                                    Command::HandleRight,
                                    Command::DeleteUntilChar(c, false),
                                ]));
                            }
                            State::di => {
                                return Some(Command::Multiple(vec![
                                    Command::MoveBackwardTillChar(c),
                                    Command::HandleRight,
                                    Command::DeleteUntilChar(c, false),
                                ]))
                            }
                            _ => (),
                        }
                        // Command Mode
                        match c {
                            'h' => Some(Command::HandleLeft),
                            'j' => Some(Command::HandleDown),
                            'k' => Some(Command::HandleUp),
                            'l' => Some(Command::HandleRight),
                            'b' => match *state {
                                State::d => Some(Command::Multiple(vec![
                                    Command::HandleCtrlLeft,
                                    Command::DeleteNextWord,
                                ])),
                                State::c => {
                                    *mode = Mode::Insert;
                                    Some(Command::Multiple(vec![
                                        Command::HandleCtrlLeft,
                                        Command::DeleteNextWord,
                                    ]))
                                }
                                _ => Some(Command::HandleCtrlLeft),
                            },
                            'w' => match *state {
                                State::d => Some(Command::DeleteNextWord),
                                State::c => {
                                    *mode = Mode::Insert;
                                    Some(Command::DeleteNextWord)
                                }
                                _ => Some(Command::HandleCtrlRight),
                            },
                            'g' => match *state {
                                State::Empty => {
                                    *state = State::g;
                                    Some(Command::Continue)
                                }
                                State::g => {
                                    reset_state!();
                                    let rows_diff = global_variables.cursor_position.1
                                        - global_variables.prompt_position.1;
                                    Some(Command::Multiple(vec![Command::HandleUp; rows_diff]))
                                }
                                _ => {
                                    reset_state!();
                                    Some(Command::Continue)
                                }
                            },
                            'G' => {
                                if *state == State::d {
                                    Some(Command::DeleteTillEnd)
                                } else {
                                    Some(Command::GoToLastRow)
                                }
                            }
                            'r' => {
                                if *state == State::Empty {
                                    *state = State::r;
                                }
                                Some(Command::Continue)
                            }
                            'x' => Some(Command::Multiple(vec![
                                Command::HandleDelete,
                                Command::PrintInput,
                            ])),
                            '$' => Some(Command::HandleEnd),
                            '^' => Some(Command::HandleHome),
                            'f' => match state {
                                State::Empty => {
                                    *state = State::f;
                                    Some(Command::Continue)
                                }
                                _ => {
                                    reset_state!();
                                    Some(Command::Continue)
                                }
                            },
                            'F' => match state {
                                State::Empty => {
                                    *state = State::F;
                                    Some(Command::Continue)
                                }
                                _ => {
                                    reset_state!();
                                    Some(Command::Continue)
                                }
                            },
                            'i' => match *state {
                                State::c => {
                                    *state = State::ci;
                                    Some(Command::Continue)
                                }
                                State::d => {
                                    *state = State::di;
                                    Some(Command::Continue)
                                }
                                _ => {
                                    *mode = Mode::Insert;
                                    Some(Command::SetThinCursor)
                                }
                            },
                            'I' => {
                                *mode = Mode::Insert;
                                let commands = vec![Command::SetThinCursor, Command::HandleHome];
                                Some(Command::Multiple(commands))
                            }
                            'o' => {
                                *mode = Mode::Insert;
                                let commands = vec![
                                    Command::SetThinCursor,
                                    Command::HandleEnd,
                                    Command::HandleAltEnter,
                                ];
                                Some(Command::Multiple(commands))
                            }
                            'a' => {
                                *mode = Mode::Insert;
                                let commands = vec![Command::SetThinCursor, Command::HandleRight];
                                Some(Command::Multiple(commands))
                            }
                            'A' => {
                                *mode = Mode::Insert;
                                let commands = vec![Command::SetThinCursor, Command::HandleEnd];
                                Some(Command::Multiple(commands))
                            }
                            'd' => match state {
                                State::Empty => {
                                    *state = State::d;
                                    Some(Command::Continue)
                                }
                                State::d => {
                                    reset_state!();
                                    Some(Command::Multiple(vec![
                                        Command::HandleHome,
                                        Command::DeleteUntilChar('\n', true),
                                    ]))
                                }
                                _ => {
                                    reset_state!();
                                    Some(Command::Continue)
                                }
                            },
                            'D' => Some(Command::DeleteUntilChar('\n', false)),
                            'c' => match state {
                                State::Empty => {
                                    *state = State::c;
                                    Some(Command::Continue)
                                }
                                State::c => {
                                    *mode = Mode::Insert;
                                    reset_state!();
                                    Some(Command::Multiple(vec![
                                        Command::HandleHome,
                                        Command::DeleteUntilChar('\n', true),
                                    ]))
                                }
                                _ => {
                                    reset_state!();
                                    Some(Command::Continue)
                                }
                            },
                            'C' => {
                                *mode = Mode::Insert;
                                Some(Command::DeleteUntilChar('\n', false))
                            }
                            _ => Some(Command::Continue),
                        }
                    }
                }
                KeyEvent {
                    code: KeyCode::Esc, ..
                } => {
                    *mode = Mode::Normal;
                    Some(Command::SetWideCursor)
                }
                _ => None,
            },
            Event::Mouse(_) => None,
            Event::Resize(_, _) => None,
        })();

        // Second match to update the state
        if !matches!(cmd, Some(Command::Continue)) {
            reset_state!()
        }

        bincode::serialize_into(std::io::stdout(), &cmd).unwrap();
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq)]
enum State {
    Empty,
    c,
    ci,
    d,
    di,
    g,
    f,
    F,
    r,
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Insert,
}