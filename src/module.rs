use crate::{Command, data::Data, error::AppError};
use poise::serenity_prelude as serenity;
use std::pin::Pin;

pub type EventFuture<'a> = Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + 'a>>;

pub trait Module: Send + Sync {
    fn name(&self) -> &'static str;
    fn commands(&self) -> Vec<Command>;
    fn setup(&self, _ctx: serenity::Context, _data: Data) {}
    fn handle_event<'a>(
        &'a self,
        _ctx: &'a serenity::Context,
        _event: &'a serenity::FullEvent,
        _data: &'a Data,
    ) -> EventFuture<'a> {
        Box::pin(async { Ok(()) })
    }
}
