//! Xnes3D+: the setup the console interface shows the first time it starts on Android: the ROM
//! folder, the saves folder, the box art and the community 3D profiles, one step each. It is
//! shown again only if its file in the user data folder goes.

use super::pad::PadButton;
use crate::android_bridge::AndroidAction;

/// The file of the user data folder that says the setup was done (or skipped).
pub const DONE_FILE: &str = "setup-done.txt";

/// What a choice of a step does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupChoice {
    /// Ask Android for this (a folder picker, a download), and stay on the step.
    Android(AndroidAction),
    /// Fetch the box art of every game from this source ("launchbox" or "esde"), then go on.
    Media(&'static str),
    /// Go on to the next step.
    Next,
}

/// One choice: its title, the line under it, and what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub title: &'static str,
    pub text: &'static str,
    pub action: SetupChoice,
}

/// One step: its heading, the paragraph under it, its choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub heading: &'static str,
    pub text: &'static str,
    pub choices: &'static [Choice],
}

/// The steps, in order.
pub static STEPS: [Step; 4] = [
    Step {
        heading: "Your games",
        text: "Choose the folder your NES and Famicom games are in. Xnes3D+ only reads it: \
               nothing is copied or moved.",
        choices: &[
            Choice {
                title: "Choose the ROM folder",
                text: "Opens Android's folder picker.",
                action: SetupChoice::Android(AndroidAction::ChooseRomFolder),
            },
            Choice {
                title: "Next",
                text: "Once the folder is chosen, or to choose it later in Options.",
                action: SetupChoice::Next,
            },
        ],
    },
    Step {
        heading: "Saves",
        text: "Battery saves and save states go in Xnes3D+'s own folder, unless you pick another \
               one, for instance to keep them with your other emulators' saves.",
        choices: &[
            Choice {
                title: "Keep the default folder",
                text: "Nothing to choose.",
                action: SetupChoice::Next,
            },
            Choice {
                title: "Choose a saves folder",
                text: "Used from the next start.",
                action: SetupChoice::Android(AndroidAction::ChooseSavesFolder),
            },
        ],
    },
    Step {
        heading: "Box art and game info",
        text: "Xnes3D+ can show box art, release years and descriptions from the LaunchBox Games \
               Database. It downloads a list of the NES and Famicom games once, then the pictures \
               of the games in your folder.",
        choices: &[
            Choice {
                title: "Download box art and info",
                text: "From the LaunchBox Games Database.",
                action: SetupChoice::Media("launchbox"),
            },
            Choice {
                title: "Use what ES-DE already downloaded",
                text: "Copies the box art and info your frontend saved. No download.",
                action: SetupChoice::Media("esde"),
            },
            Choice {
                title: "No box art",
                text: "The list stays text only. You can get it later in Options.",
                action: SetupChoice::Next,
            },
        ],
    },
    Step {
        heading: "3D profiles",
        text: "A 3D profile turns a game into 3D. Games without one play in Smart 3D. The \
               community shares profiles for many games.",
        choices: &[
            Choice {
                title: "Get the community profiles",
                text: "Each one comes from its maker's own link.",
                action: SetupChoice::Android(AndroidAction::CommunityProfiles),
            },
            Choice {
                title: "Done",
                text: "To the game list.",
                action: SetupChoice::Next,
            },
        ],
    },
];

/// What a press asks of the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupCommand {
    /// Carry out this choice.
    Choose(SetupChoice),
    /// The setup is over (finished or skipped).
    Finish,
}

/// The step shown and its selected choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SetupModel {
    step: usize,
    selected: usize,
}

impl SetupModel {
    /// The step shown.
    #[must_use]
    pub fn step(&self) -> &'static Step {
        &STEPS[self.step.min(STEPS.len() - 1)]
    }

    /// Its number, from 1.
    #[must_use]
    pub const fn number(&self) -> usize {
        self.step + 1
    }

    /// The selected choice.
    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// The next step, or the end of the setup after the last one.
    pub fn next(&mut self) -> Option<SetupCommand> {
        if self.step + 1 >= STEPS.len() {
            return Some(SetupCommand::Finish);
        }
        self.step += 1;
        self.selected = 0;
        None
    }

    /// One press of a button.
    pub fn handle(&mut self, button: PadButton) -> Option<SetupCommand> {
        let count = self.step().choices.len();
        match button {
            PadButton::Up => self.selected = (self.selected + count - 1) % count,
            PadButton::Down => self.selected = (self.selected + 1) % count,
            PadButton::A => {
                return Some(SetupCommand::Choose(self.step().choices[self.selected].action));
            }
            PadButton::B if self.step > 0 => {
                self.step -= 1;
                self.selected = 0;
            }
            PadButton::Start => return Some(SetupCommand::Finish),
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_steps_go_one_after_the_other_and_end() {
        let mut model = SetupModel::default();
        assert_eq!(1, model.number());
        for _ in 1..STEPS.len() {
            assert_eq!(None, model.next());
        }
        assert_eq!(STEPS.len(), model.number());
        assert_eq!(Some(SetupCommand::Finish), model.next());
        assert_eq!(None, model.handle(PadButton::B));
        assert_eq!(STEPS.len() - 1, model.number());
        assert_eq!(Some(SetupCommand::Finish), model.handle(PadButton::Start));
    }
}
