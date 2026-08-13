use crate::{
    Context,
    error::AppError,
    modules::voice::setup::{self, TrackMeta},
};
use std::collections::VecDeque;

fn shift<T>(queue: &mut VecDeque<T>, from: usize, to: usize) -> bool {
    let Some(item) = queue.remove(from) else {
        return false;
    };

    queue.insert(to.min(queue.len()), item);
    true
}

/// Move a queued track to a different position
#[poise::command(slash_command, guild_only, rename = "move")]
pub async fn move_track(
    ctx: Context<'_>,
    #[description = "Position to move, as shown by /queue"]
    #[min = 1]
    from: u32,
    #[description = "Position to move it to"]
    #[min = 1]
    to: u32,
) -> Result<(), AppError> {
    setup::require_dj(ctx).await?;

    if from == to {
        return Err(AppError::Message(
            "That track is already in that position.".into(),
        ));
    }

    let call = setup::current_call(ctx).await?;
    let (from_index, to_index) = (from as usize, to as usize);

    let title = {
        let call = call.lock().await;
        let queue = call.queue();

        let upcoming = queue.len().saturating_sub(1);
        if upcoming < 2 {
            return Err(AppError::Message(
                "There aren't enough queued tracks to reorder.".into(),
            ));
        }

        for (label, value) in [("from", from), ("to", to)] {
            if value as usize > upcoming {
                return Err(AppError::Message(format!(
                    "`{label}` must be between 1 and {upcoming}."
                )));
            }
        }

        queue.modify_queue(|q| {
            let title = q.get(from_index)?.data::<TrackMeta>().title.clone();
            shift(q, from_index, to_index).then_some(title)
        })
    };

    let title = title.ok_or_else(|| AppError::Message("That track just went away.".into()))?;

    ctx.send(setup::safe_reply(format!(
        "Moved **{title}** to position #{to}."
    )))
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue() -> VecDeque<&'static str> {
        VecDeque::from(["playing", "a", "b", "c", "d"])
    }

    #[test]
    fn moves_a_track_forward() {
        let mut q = queue();
        assert!(shift(&mut q, 1, 3));
        assert_eq!(Vec::from(q), ["playing", "b", "c", "a", "d"]);
    }

    #[test]
    fn moves_a_track_backward() {
        let mut q = queue();
        assert!(shift(&mut q, 4, 1));
        assert_eq!(Vec::from(q), ["playing", "d", "a", "b", "c"]);
    }

    #[test]
    fn moving_to_the_last_position_keeps_it_last() {
        let mut q = queue();
        assert!(shift(&mut q, 1, 4));
        assert_eq!(Vec::from(q), ["playing", "b", "c", "d", "a"]);
    }

    #[test]
    fn never_disturbs_the_playing_track() {
        let mut q = queue();
        shift(&mut q, 3, 1);
        assert_eq!(q.front(), Some(&"playing"));
    }

    #[test]
    fn out_of_range_is_rejected() {
        let mut q = queue();
        assert!(!shift(&mut q, 9, 1));
        assert_eq!(Vec::from(q), ["playing", "a", "b", "c", "d"]);
    }
}
