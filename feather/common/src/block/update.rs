use base::BlockPosition;
use ecs::SysResult;
use libcraft_core::BlockFace;

use crate::{events::BlockChangeEvent, Game};

use super::util::connect_neighbours_and_up;

/// TODO: send updated blocks to player
pub fn block_update(game: &mut Game) -> SysResult {
    for (_, event) in game.ecs.query::<&BlockChangeEvent>().iter() {
        for pos in event.iter_changed_blocks().map(Into::<BlockPosition>::into) {
            for adjacent in [
                BlockFace::East,
                BlockFace::West,
                BlockFace::North,
                BlockFace::South,
            ]
            .iter()
            .map(|&d| pos.adjacent(d))
            {
                if connect_neighbours_and_up(&mut game.world, adjacent).is_none() {
                    continue;
                }
            }
        }
    }
    Ok(())
}
