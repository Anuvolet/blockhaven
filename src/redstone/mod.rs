//! Redstone: dust with decaying power, torches (inverters), levers, buttons, pressure plates,
//! lamps, pistons, powered doors and TNT. Event-driven: changed positions are queued and
//! re-evaluated on the fixed 20 TPS tick.

use crate::world::block::{self, face_offset, voxel, Block};
use crate::world::World;
use std::collections::{HashMap, HashSet};

pub const BUTTON_TICKS: u32 = 20;
pub const MAX_PUSH: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RsEvent {
    PrimeTnt((i32, i32, i32)),
    Piston((i32, i32, i32)),
    Door((i32, i32, i32)),
    Click((i32, i32, i32)),
}

#[derive(Default)]
pub struct Redstone {
    dirty: HashSet<(i32, i32, i32)>,
    buttons: HashMap<(i32, i32, i32), u32>,
    pub pressed_plates: HashSet<(i32, i32, i32)>,
}

const DIRS: [(i32, i32, i32); 6] = [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)];

fn add(p: (i32, i32, i32), d: (i32, i32, i32)) -> (i32, i32, i32) {
    (p.0 + d.0, p.1 + d.1, p.2 + d.2)
}

/// Block a torch is attached to (relative offset).
pub fn torch_attachment(meta: u8) -> (i32, i32, i32) {
    match meta & 7 {
        1 => (1, 0, 0),
        2 => (-1, 0, 0),
        3 => (0, 0, 1),
        4 => (0, 0, -1),
        _ => (0, -1, 0),
    }
}

/// Block a button / lever is attached to (relative offset).
pub fn button_attachment(meta: u8) -> (i32, i32, i32) {
    face_offset(meta & 7)
}

fn is_conductor(v: u16) -> bool {
    let b = block::vox_block(v);
    let p = block::props(b.id());
    p.opaque && p.solid && !matches!(b, Block::RedstoneLamp | Block::RedstoneLampLit | Block::Piston | Block::StickyPiston | Block::Tnt)
}

/// Strong power of a conductor block at `pos` (0 or 15): torch below, attached lever/button on,
/// pressure plate on top.
fn strong_power(world: &World, pos: (i32, i32, i32)) -> u8 {
    for d in DIRS {
        let n = add(pos, d);
        let v = world.get(n.0, n.1, n.2);
        let b = block::vox_block(v);
        let m = block::vox_meta(v);
        match b {
            Block::RedstoneTorchOn => {
                // a floor torch strongly powers the block directly above it
                if d == (0, -1, 0) && torch_attachment(m) == (0, -1, 0) {
                    return 15;
                }
            }
            Block::Lever | Block::Button => {
                if m & 8 != 0 && add(n, button_attachment(m)) == pos {
                    return 15;
                }
            }
            Block::PressurePlate => {
                if m & 1 != 0 && d == (0, 1, 0) {
                    return 15;
                }
            }
            _ => {}
        }
    }
    0
}

/// Weak power of a conductor: strong power or adjacent powered dust (sideways or on top).
fn weak_power(world: &World, pos: (i32, i32, i32)) -> u8 {
    let s = strong_power(world, pos);
    if s > 0 {
        return s;
    }
    for d in [(-1, 0, 0), (1, 0, 0), (0, 0, -1), (0, 0, 1), (0, 1, 0)] {
        let n = add(pos, d);
        let v = world.get(n.0, n.1, n.2);
        if block::vox_block(v) == Block::RedstoneDust && block::vox_meta(v) > 0 {
            return 15;
        }
    }
    0
}

/// Power a neighbour `n` (in direction `d` from `pos`) delivers into `pos`.
/// `for_dust`: dust only accepts strong block power and decayed dust power.
fn power_from(world: &World, pos: (i32, i32, i32), n: (i32, i32, i32), d: (i32, i32, i32), for_dust: bool) -> u8 {
    let v = world.get(n.0, n.1, n.2);
    let b = block::vox_block(v);
    let m = block::vox_meta(v);
    match b {
        Block::RedstoneTorchOn => {
            if add(n, torch_attachment(m)) == pos {
                0
            } else {
                15
            }
        }
        Block::Lever | Block::Button => {
            if m & 8 != 0 { 15 } else { 0 }
        }
        Block::PressurePlate => {
            if m & 1 != 0 { 15 } else { 0 }
        }
        Block::RedstoneDust => {
            // dust does not power the block above it
            if d == (0, -1, 0) {
                return 0;
            }
            let p = m & 15;
            if for_dust {
                p.saturating_sub(1)
            } else if p > 0 {
                15
            } else {
                0
            }
        }
        _ => {
            if is_conductor(v) {
                if for_dust { strong_power(world, n) } else { weak_power(world, n) }
            } else {
                0
            }
        }
    }
}

pub fn input_power(world: &World, pos: (i32, i32, i32), for_dust: bool) -> u8 {
    let mut best = 0;
    for d in DIRS {
        let n = add(pos, d);
        best = best.max(power_from(world, pos, n, d, for_dust));
    }
    best
}

/// Can this block be moved by a piston?
fn movable(v: u16) -> bool {
    if v == 0 {
        return false;
    }
    let b = block::vox_block(v);
    let p = block::props(b.id());
    if p.hardness < 0.0 || block::is_fluid(v) {
        return false;
    }
    !matches!(b, Block::Obsidian | Block::Chest | Block::Furnace | Block::FurnaceLit | Block::Spawner | Block::PistonHead | Block::Bed | Block::Door)
        && !(matches!(b, Block::Piston | Block::StickyPiston) && block::vox_meta(v) & 8 != 0)
}

impl Redstone {
    pub fn new() -> Redstone {
        Redstone::default()
    }

    /// Mark a position and its neighbours for re-evaluation.
    pub fn mark(&mut self, pos: (i32, i32, i32)) {
        self.dirty.insert(pos);
        for d in DIRS {
            self.dirty.insert(add(pos, d));
        }
    }

    pub fn press_button(&mut self, pos: (i32, i32, i32)) {
        self.buttons.insert(pos, BUTTON_TICKS);
        self.mark(pos);
    }

    pub fn pending(&self) -> usize {
        self.dirty.len()
    }

    /// One tick: re-evaluate dirty positions (bounded), release buttons.
    pub fn step(&mut self, world: &World) -> Vec<RsEvent> {
        let mut events = Vec::new();
        // buttons
        let mut released = Vec::new();
        for (p, t) in self.buttons.iter_mut() {
            *t = t.saturating_sub(1);
            if *t == 0 {
                released.push(*p);
            }
        }
        for p in released {
            self.buttons.remove(&p);
            let v = world.get(p.0, p.1, p.2);
            if block::vox_block(v) == Block::Button {
                world.set_block(p.0, p.1, p.2, voxel(Block::Button, block::vox_meta(v) & !8));
                events.push(RsEvent::Click(p));
            }
            self.mark(p);
        }
        let mut queue: Vec<(i32, i32, i32)> = self.dirty.drain().collect();
        let mut relayed: HashSet<(i32, i32, i32)> = HashSet::new();
        let mut processed = 0;
        while let Some(pos) = queue.pop() {
            processed += 1;
            if processed > 4096 {
                // leave the rest for the next tick
                for q in queue {
                    self.dirty.insert(q);
                }
                break;
            }
            let v = world.get(pos.0, pos.1, pos.2);
            let b = block::vox_block(v);
            let m = block::vox_meta(v);
            // a conductor passes the change on to the components touching it (never to other
            // conductors, or the whole ground would be re-evaluated)
            if is_conductor(v) {
                if relayed.insert(pos) {
                    for d in DIRS {
                        let n = add(pos, d);
                        if !is_conductor(world.get(n.0, n.1, n.2)) && !queue.contains(&n) {
                            queue.push(n);
                        }
                    }
                }
                continue;
            }
            let mut changed: Option<u16> = None;
            match b {
                Block::RedstoneDust => {
                    let p = input_power(world, pos, true);
                    if p != (m & 15) {
                        changed = Some(voxel(b, p));
                    }
                }
                Block::RedstoneTorchOn | Block::RedstoneTorchOff => {
                    let att = add(pos, torch_attachment(m));
                    let av = world.get(att.0, att.1, att.2);
                    let powered = if is_conductor(av) {
                        weak_power(world, att) > 0
                    } else {
                        false
                    };
                    let want_on = !powered;
                    if want_on != (b == Block::RedstoneTorchOn) {
                        changed = Some(voxel(if want_on { Block::RedstoneTorchOn } else { Block::RedstoneTorchOff }, m));
                    }
                }
                Block::RedstoneLamp | Block::RedstoneLampLit => {
                    let lit = input_power(world, pos, false) > 0;
                    if lit != (b == Block::RedstoneLampLit) {
                        changed = Some(voxel(if lit { Block::RedstoneLampLit } else { Block::RedstoneLamp }, 0));
                    }
                }
                Block::Door => {
                    let lower = if m & 8 != 0 { (pos.0, pos.1 - 1, pos.2) } else { pos };
                    let upper = (lower.0, lower.1 + 1, lower.2);
                    let powered = input_power(world, lower, false) > 0 || input_power(world, upper, false) > 0;
                    let was = m & 16 != 0;
                    if powered != was {
                        let open = if powered { 4 } else { 0 };
                        let pw = if powered { 16 } else { 0 };
                        for (p, up) in [(lower, 0u8), (upper, 8u8)] {
                            let pv = world.get(p.0, p.1, p.2);
                            if block::vox_block(pv) == Block::Door {
                                let pm = block::vox_meta(pv);
                                world.set_block(p.0, p.1, p.2, voxel(Block::Door, (pm & 3) | open | up | pw));
                            }
                        }
                        events.push(RsEvent::Door(lower));
                    }
                }
                Block::Piston | Block::StickyPiston => {
                    let powered = input_power(world, pos, false) > 0;
                    let extended = m & 8 != 0;
                    if powered && !extended {
                        if self.extend(world, pos, b, m) {
                            events.push(RsEvent::Piston(pos));
                            self.mark(pos);
                            let dir = face_offset(m & 7);
                            for k in 1..=MAX_PUSH as i32 + 1 {
                                self.mark((pos.0 + dir.0 * k, pos.1 + dir.1 * k, pos.2 + dir.2 * k));
                            }
                        }
                    } else if !powered && extended {
                        self.retract(world, pos, b, m);
                        events.push(RsEvent::Piston(pos));
                        self.mark(pos);
                        let dir = face_offset(m & 7);
                        self.mark((pos.0 + dir.0 * 2, pos.1 + dir.1 * 2, pos.2 + dir.2 * 2));
                    }
                }
                Block::Tnt => {
                    if input_power(world, pos, false) > 0 {
                        world.set_block(pos.0, pos.1, pos.2, 0);
                        events.push(RsEvent::PrimeTnt(pos));
                    }
                }
                _ => {}
            }
            if let Some(nv) = changed {
                world.set_block(pos.0, pos.1, pos.2, nv);
                for d in DIRS {
                    let n = add(pos, d);
                    if !queue.contains(&n) {
                        queue.push(n);
                    }
                }
                // conductors relay changes to the components on their other sides
                for d in DIRS {
                    let n = add(pos, d);
                    if is_conductor(world.get(n.0, n.1, n.2)) {
                        for d2 in DIRS {
                            let n2 = add(n, d2);
                            if n2 != pos && !is_conductor(world.get(n2.0, n2.1, n2.2)) && !queue.contains(&n2) {
                                queue.push(n2);
                            }
                        }
                    }
                }
            }
        }
        events
    }

    fn extend(&mut self, world: &World, pos: (i32, i32, i32), b: Block, m: u8) -> bool {
        let dir = face_offset(m & 7);
        // collect the column of blocks to push
        let mut chain = Vec::new();
        let mut p = add(pos, dir);
        loop {
            let v = world.get(p.0, p.1, p.2);
            let props = block::props(block::vox_id(v));
            if v == 0 || props.replaceable {
                break;
            }
            if !movable(v) || chain.len() >= MAX_PUSH {
                return false;
            }
            chain.push((p, v));
            p = add(p, dir);
        }
        if !(0..256).contains(&p.1) {
            return false;
        }
        // move from the far end
        for (bp, bv) in chain.iter().rev() {
            let np = add(*bp, dir);
            world.set_block(np.0, np.1, np.2, *bv);
            self.mark(np);
        }
        let head = add(pos, dir);
        let sticky = if b == Block::StickyPiston { 8 } else { 0 };
        world.set_block(head.0, head.1, head.2, voxel(Block::PistonHead, (m & 7) | sticky));
        world.set_block(pos.0, pos.1, pos.2, voxel(b, m | 8));
        true
    }

    fn retract(&mut self, world: &World, pos: (i32, i32, i32), b: Block, m: u8) {
        let dir = face_offset(m & 7);
        let head = add(pos, dir);
        if world.get_block(head.0, head.1, head.2) == Block::PistonHead {
            world.set_block(head.0, head.1, head.2, 0);
        }
        world.set_block(pos.0, pos.1, pos.2, voxel(b, m & !8));
        if b == Block::StickyPiston {
            let far = add(head, dir);
            let fv = world.get(far.0, far.1, far.2);
            if movable(fv) {
                world.set_block(far.0, far.1, far.2, 0);
                world.set_block(head.0, head.1, head.2, fv);
                self.mark(far);
                self.mark(head);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::Chunk;

    fn flat_world() -> std::sync::Arc<World> {
        let w = World::new(0);
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut c = Chunk::new(cx, cz);
                for z in 0..16 {
                    for x in 0..16 {
                        for y in 0..10 {
                            c.set(x, y, z, voxel(Block::Stone, 0));
                        }
                    }
                }
                c.recompute_heightmap();
                crate::world::light::init_chunk_light(&mut c);
                w.insert_chunk(c);
            }
        }
        w
    }

    fn settle(rs: &mut Redstone, w: &World, ticks: usize) -> Vec<RsEvent> {
        let mut ev = Vec::new();
        for _ in 0..ticks {
            ev.extend(rs.step(w));
        }
        ev
    }

    #[test]
    fn dust_decays_away_from_a_torch() {
        let w = flat_world();
        let mut rs = Redstone::new();
        w.set_block(0, 10, 8, voxel(Block::RedstoneTorchOn, 0));
        for x in 1..=16 {
            w.set_block(x, 10, 8, voxel(Block::RedstoneDust, 0));
        }
        for x in 0..=16 {
            rs.mark((x, 10, 8));
        }
        settle(&mut rs, &w, 40);
        assert_eq!(block::vox_meta(w.get(1, 10, 8)), 15);
        assert_eq!(block::vox_meta(w.get(5, 10, 8)), 11);
        assert_eq!(block::vox_meta(w.get(15, 10, 8)), 1);
        assert_eq!(block::vox_meta(w.get(16, 10, 8)), 0);
        // removing the torch drains the line
        w.set_block(0, 10, 8, 0);
        rs.mark((0, 10, 8));
        settle(&mut rs, &w, 40);
        assert_eq!(block::vox_meta(w.get(1, 10, 8)), 0);
        assert_eq!(block::vox_meta(w.get(8, 10, 8)), 0);
    }

    #[test]
    fn lever_powers_a_lamp_through_a_block_and_torch_inverts() {
        let w = flat_world();
        let mut rs = Redstone::new();
        // lever on the +X face of a stone block at (5,10,5); lamp on the -X side; torch on top
        w.set_block(5, 10, 5, voxel(Block::Stone, 0));
        w.set_block(6, 10, 5, voxel(Block::Lever, 0)); // face 0 -> support at -X = (5,10,5)
        w.set_block(4, 10, 5, voxel(Block::RedstoneLamp, 0));
        w.set_block(5, 11, 5, voxel(Block::RedstoneTorchOn, 0));
        for p in [(5, 10, 5), (6, 10, 5), (4, 10, 5), (5, 11, 5)] {
            rs.mark(p);
        }
        settle(&mut rs, &w, 10);
        assert_eq!(w.get_block(4, 10, 5), Block::RedstoneLamp);
        assert_eq!(w.get_block(5, 11, 5), Block::RedstoneTorchOn);
        // flip the lever
        w.set_block(6, 10, 5, voxel(Block::Lever, 8));
        rs.mark((6, 10, 5));
        settle(&mut rs, &w, 10);
        assert_eq!(w.get_block(4, 10, 5), Block::RedstoneLampLit);
        assert_eq!(w.get_block(5, 11, 5), Block::RedstoneTorchOff);
        w.set_block(6, 10, 5, voxel(Block::Lever, 0));
        rs.mark((6, 10, 5));
        settle(&mut rs, &w, 10);
        assert_eq!(w.get_block(4, 10, 5), Block::RedstoneLamp);
        assert_eq!(w.get_block(5, 11, 5), Block::RedstoneTorchOn);
    }

    #[test]
    fn piston_pushes_a_block_and_sticky_pulls_it_back() {
        let w = flat_world();
        let mut rs = Redstone::new();
        // sticky piston facing +X at (5,10,5), cobble in front, lever behind it on a block
        w.set_block(5, 10, 5, voxel(Block::StickyPiston, 1));
        w.set_block(6, 10, 5, voxel(Block::Cobblestone, 0));
        w.set_block(5, 10, 6, voxel(Block::Lever, 4 | 8)); // face 4 -> support at -Z = (5,10,5)
        rs.mark((5, 10, 5));
        rs.mark((5, 10, 6));
        let ev = settle(&mut rs, &w, 5);
        assert!(ev.contains(&RsEvent::Piston((5, 10, 5))));
        assert_eq!(w.get_block(6, 10, 5), Block::PistonHead);
        assert_eq!(w.get_block(7, 10, 5), Block::Cobblestone);
        assert!(block::vox_meta(w.get(5, 10, 5)) & 8 != 0);
        w.set_block(5, 10, 6, voxel(Block::Lever, 4));
        rs.mark((5, 10, 6));
        settle(&mut rs, &w, 5);
        assert_eq!(w.get_block(6, 10, 5), Block::Cobblestone);
        assert_eq!(w.get_block(7, 10, 5), Block::Air);
    }

    #[test]
    fn button_releases_and_tnt_primes() {
        let w = flat_world();
        let mut rs = Redstone::new();
        w.set_block(5, 10, 5, voxel(Block::Stone, 0));
        w.set_block(6, 10, 5, voxel(Block::Button, 8));
        w.set_block(4, 10, 5, voxel(Block::Tnt, 0));
        rs.press_button((6, 10, 5));
        rs.mark((4, 10, 5));
        let mut ev = Vec::new();
        for _ in 0..(BUTTON_TICKS as usize + 2) {
            ev.extend(rs.step(&w));
        }
        assert!(ev.contains(&RsEvent::PrimeTnt((4, 10, 5))));
        assert_eq!(w.get_block(4, 10, 5), Block::Air);
        assert_eq!(block::vox_meta(w.get(6, 10, 5)) & 8, 0, "button should release");
    }
}
