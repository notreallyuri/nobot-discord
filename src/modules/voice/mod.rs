use crate::{data::Data, module::Module};
use poise::serenity_prelude as serenity;

pub mod announce;
pub mod commands;
pub mod idle;
pub mod lyrics;
pub mod setup;
pub mod sources;
pub mod spotify;
pub mod suggest;
pub mod ytdlp;

pub struct VoiceModule;

impl Module for VoiceModule {
    fn name(&self) -> &'static str {
        "Voice"
    }

    fn commands(&self) -> Vec<crate::Command> {
        vec![
            commands::play(),
            commands::pause(),
            commands::resume(),
            commands::skip(),
            commands::stop(),
            commands::queue(),
            commands::nowplaying(),
            commands::lyrics(),
            commands::clear(),
            commands::remove(),
            commands::move_track(),
            commands::leave(),
        ]
    }

    fn setup(&self, ctx: serenity::Context, data: Data) {
        idle::watch(ctx, data);
    }
}
