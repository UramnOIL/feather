//! A bunch of math-related functions for use with
//! the physics system.

use crate::entity::{ChunkEntities, EntityComponent};
use feather_core::world::block::Block;
use feather_core::world::{BlockPosition, ChunkMap, Position};
use feather_core::ChunkPosition;
use glm::{vec3, DVec3, Vec3};
use smallvec::SmallVec;
use specs::storage::GenericReadStorage;
use specs::Entity;
use std::f32::INFINITY;

/// Finds the first block impacted by the given ray.
///
/// Traces up to `max_distance` before returning `None`
/// if no block was found.
pub fn block_impacted_by_ray(
    chunk_map: &ChunkMap,
    origin: Vec3,
    ray: Vec3,
    max_distance_squared: f32,
) -> Option<BlockPosition> {
    assert_ne!(ray, vec3(0.0, 0.0, 0.0));

    // Go along path of ray and find all points
    // where one or more coordinates are integers.
    // Any position with an integer component
    // is a block boundary, which means a block
    // could be found at the position.
    //
    // This algorithm is based on "A Fast Voxel Traversal Algorithm for Ray Tracing"
    // by John Amanatides and Andrew Woo and has been adapted
    // to our purposes.

    let direction = ray.normalize();

    let mut dist_traveled = glm::vec3(0.0f32, 0.0, 0.0);

    let mut step = glm::vec3(0, 0, 0);
    let mut delta = glm::vec3(INFINITY, INFINITY, INFINITY);
    let mut next = glm::vec3(INFINITY, INFINITY, INFINITY);

    if direction.x > 0.0 {
        step.x = 1;
        delta.x = 1.0 / direction.x;
        next.x = ((origin.x + 1.0).floor() - origin.x) / direction.x; // Brings X position to next integer
    } else if direction.x < 0.0 {
        step.x = -1;
        delta.x = (1.0 / direction.x).abs();
        next.x = ((origin.x - (origin.x - 1.0).ceil()) / direction.x).abs();
    }

    if direction.y > 0.0 {
        step.y = 1;
        delta.y = 1.0 / direction.y;
        next.y = ((origin.y + 1.0).floor() - origin.y) / direction.y;
    } else if direction.y < 0.0 {
        step.y = -1;
        delta.y = (1.0 / direction.y).abs();
        next.y = ((origin.y - (origin.y - 1.0).ceil()) / direction.y).abs();
    }

    if direction.z > 0.0 {
        step.z = 1;
        delta.z = 1.0 / direction.z;
        next.z = ((origin.z + 1.0).floor() - origin.z) / direction.z;
    } else if direction.z < 0.0 {
        step.z = -1;
        delta.z = (1.0 / direction.z).abs();
        next.z = ((origin.z - (origin.z - 1.0).ceil()) / direction.z).abs();
    }

    let mut current_pos = Position::from(origin).block_pos();

    while dist_traveled.magnitude_squared() < max_distance_squared {
        if let Some(block) = chunk_map.block_at(current_pos) {
            if block != Block::Air {
                return Some(current_pos);
            }
        } else {
            // Traveled outside loaded chunks - no blocks found
            return None;
        }

        if next.x < next.y {
            if next.x < next.z {
                next.x += delta.x;
                current_pos.x += step.x;
                dist_traveled.x += 1.0;
            } else {
                next.z += delta.z;
                current_pos.z += step.z;
                dist_traveled.z += 1.0;
            }
        } else if next.y < next.z {
            next.y += delta.y;
            current_pos.y += step.y;
            dist_traveled.y += 1.0;
        } else {
            next.z += delta.z;
            current_pos.z += step.z;
            dist_traveled.z += 1.0;
        }
    }

    None
}

/// Returns all entities within the given distance of the given
/// position.
///
/// # Panics
/// Panics if either coordinate of the radius is negative.
pub fn nearby_entities<S>(
    chunk_entities: &ChunkEntities,
    positions: &S,
    pos: Position,
    radius: DVec3,
) -> SmallVec<[Entity; 4]>
where
    S: GenericReadStorage<Component = EntityComponent>,
{
    assert!(radius.x >= 0.0);
    assert!(radius.y >= 0.0);
    assert!(radius.z >= 0.0);

    let mut result = smallvec![];

    for chunk in chunks_within_distance(pos, radius) {
        let entities = chunk_entities.entities_in_chunk(chunk);
        entities
            .iter()
            .copied()
            .filter(|e| {
                let epos = positions.get(*e);
                if let Some(epos) = epos {
                    let epos = epos.position;
                    (epos.x - pos.x).abs() <= radius.x
                        && (epos.y - pos.y).abs() <= radius.y
                        && (epos.z - pos.z).abs() <= radius.z
                } else {
                    false
                }
            })
            .for_each(|e| result.push(e));
    }

    result
}

/// Finds all chunks within a given distance (in blocks)
/// of a position.
///
/// The Y coordinate of `distance` is ignored.
fn chunks_within_distance(mut pos: Position, mut distance: DVec3) -> SmallVec<[ChunkPosition; 9]> {
    assert!(distance.x >= 0.0);
    assert!(distance.z >= 0.0);

    let mut result = smallvec![];

    let mut x_len = 0;
    let mut z_len = 0;

    let center_chunk_pos = pos.chunk_pos();

    loop {
        let needed = ((pos.x + 16.0) / 16.0).floor() * 16.0 - pos.x;
        if needed > distance.x {
            break;
        }

        distance.x -= needed;
        pos.x += needed;
        x_len += 1;
    }

    loop {
        let needed = ((pos.z + 16.0) / 16.0).floor() * 16.0 - pos.z;
        if needed > distance.z {
            break;
        }

        distance.z -= needed;
        pos.z += needed;
        z_len += 1;
    }

    for x in -x_len..=x_len {
        for z in -z_len..=z_len {
            result.push(ChunkPosition::new(
                x + center_chunk_pos.x,
                z + center_chunk_pos.z,
            ));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityType;
    use crate::testframework as t;
    use feather_core::world::chunk::Chunk;
    use feather_core::world::ChunkPosition;
    use specs::WorldExt;
    use std::collections::HashSet;

    #[test]
    fn test_block_impacted_by_ray() {
        let mut map = chunk_map();

        assert_eq!(
            block_impacted_by_ray(&map, vec3(0.0, 65.0, 0.0), vec3(0.0, -1.0, 0.0), 5.0),
            Some(BlockPosition::new(0, 64, 0))
        );

        assert_eq!(
            block_impacted_by_ray(&map, vec3(0.0, 65.0, 0.0), vec3(0.0, 1.0, 0.0), 256.0,),
            None
        );

        assert_eq!(
            block_impacted_by_ray(&map, vec3(0.0, 70.0, 0.0), vec3(0.0, -1.0, 0.0), 5.0,),
            None
        );

        map.set_block_at(BlockPosition::new(1, 65, 1), Block::Stone)
            .unwrap();

        assert_eq!(
            block_impacted_by_ray(&map, vec3(0.0, 66.0, 0.0), vec3(1.0, -1.0, 1.0), 5.0),
            Some(BlockPosition::new(1, 65, 1))
        );
    }

    fn chunk_map() -> ChunkMap {
        let mut map = ChunkMap::new();

        for x in -2..=2 {
            for z in -2..=2 {
                let pos = ChunkPosition::new(x, z);
                let mut chunk = Chunk::new(pos);

                for x in 0..16 {
                    for y in 0..=64 {
                        for z in 0..16 {
                            chunk.set_block_at(x, y, z, Block::Stone);
                        }
                    }
                }
                map.set_chunk_at(pos, chunk);
            }
        }

        map
    }

    #[test]
    fn test_nearby_entities() {
        let (mut w, mut d) = t::init_world();

        t::populate_with_air(&mut w); // Prevents entities from getting despawned for being outside loaded chunks

        let e1 = t::add_entity_with_pos(&mut w, EntityType::Player, position!(0.0, 0.0, 0.0), true);
        let e2 = t::add_entity_with_pos(
            &mut w,
            EntityType::Player,
            position!(-100.0, 0.0, 50.0),
            true,
        );
        let e3 = t::add_entity_with_pos(
            &mut w,
            EntityType::Player,
            position!(100.0, 50.0, 50.0),
            true,
        );
        let e4 = t::add_entity_with_pos(
            &mut w,
            EntityType::Player,
            position!(100.0, 1.0, -50.0),
            true,
        );

        d.dispatch(&w);
        w.maintain();

        let entities = nearby_entities(
            &w.fetch(),
            &w.read_component(),
            position!(0.0, 0.0, 0.0),
            vec3(100.0, 1.0, 50.0),
        )
        .into_iter()
        .collect::<HashSet<_>>();

        assert_eq!(entities.len(), 3);

        assert!(entities.contains(&e1));
        assert!(entities.contains(&e2));
        assert!(!entities.contains(&e3));
        assert!(entities.contains(&e4));
    }

    #[test]
    fn test_chunks_within_distance_basic() {
        let pos = position!(0.0, 0.0, 0.0);
        let distance = vec3(16.0, 0.0, 16.0);

        let chunks = chunks_within_distance(pos, distance);

        dbg!(chunks.clone());

        let set = chunks.into_iter().collect::<HashSet<_>>();

        for x in -1..=1 {
            for z in -1..=1 {
                assert!(set.contains(&ChunkPosition::new(x, z)));
            }
        }

        assert_eq!(set.len(), 9);
    }

    #[test]
    fn test_chunks_within_distance_complex() {
        let pos = position!(32.0, 0.0, -32.0);

        let distance = vec3(32.0, 0.0, 31.0);

        let chunks = chunks_within_distance(pos, distance);

        dbg!(chunks.clone());
        assert_eq!(chunks.len(), 15);

        let set = chunks.into_iter().collect::<HashSet<_>>();

        for x in 0..=4 {
            for z in -3..=-1 {
                assert!(set.contains(&ChunkPosition::new(x, z)));
            }
        }
    }

    #[test]
    #[should_panic]
    fn test_chunks_within_distance_negative_distance() {
        let pos = position!(16.0, 0.0, 16.0);
        let distance = vec3(-0.1, -50.0, 0.0);
        chunks_within_distance(pos, distance);
    }
}