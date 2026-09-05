//! Block registry: ids, static properties, and texture tile mapping.
//!
//! A voxel is a `u16`: low byte = block id, high byte = metadata.

use crate::render::atlas::Tile;
use std::sync::OnceLock;

macro_rules! blocks {
    ($($name:ident = $id:expr),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[repr(u8)]
        pub enum Block { $($name = $id),* }
        impl Block {
            pub const ALL: &'static [Block] = &[$(Block::$name),*];
            pub fn from_id(id: u8) -> Block {
                match id { $($id => Block::$name,)* _ => Block::Air }
            }
        }
    };
}

blocks! {
    Air = 0, Stone = 1, Grass = 2, Dirt = 3, Cobblestone = 4, OakPlanks = 5, BirchPlanks = 6,
    SprucePlanks = 7, Bedrock = 8, Water = 9, Lava = 10, Sand = 11, Gravel = 12, CoalOre = 13,
    IronOre = 14, GoldOre = 15, DiamondOre = 16, RedstoneOre = 17, OakLog = 18, BirchLog = 19,
    SpruceLog = 20, OakLeaves = 21, BirchLeaves = 22, SpruceLeaves = 23, Glass = 24,
    Sandstone = 25, Snow = 26, Ice = 27, Cactus = 28, Clay = 29, TallGrass = 30, DeadBush = 31,
    Dandelion = 32, Poppy = 33, BrownMushroom = 34, RedMushroom = 35, CraftingTable = 36,
    Furnace = 37, FurnaceLit = 38, Chest = 39, Torch = 40, Ladder = 41, Door = 42, Bed = 43,
    RedstoneDust = 44, RedstoneTorchOn = 45, RedstoneTorchOff = 46, Lever = 47, Button = 48,
    PressurePlate = 49, RedstoneLamp = 50, RedstoneLampLit = 51, Piston = 52, StickyPiston = 53,
    PistonHead = 54, Tnt = 55, Glowstone = 56, MossyCobblestone = 57, Spawner = 58,
    Obsidian = 59, Wool = 60, StoneBricks = 61, CrackedStoneBricks = 62, IronBlock = 63,
    GoldBlock = 64, DiamondBlock = 65, MushroomStem = 66, RedMushroomBlock = 67,
    BrownMushroomBlock = 68, Farmland = 69, Wheat = 70, Podzol = 71, SnowyGrass = 72,
    Bricks = 73, Bookshelf = 74, HayBale = 75,
}

impl Block {
    #[inline]
    pub fn id(self) -> u8 {
        self as u8
    }
}

/// Geometry family used by the mesher and the physics.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Full opaque or cut-out cube.
    Cube,
    /// Two crossed quads (grass, flowers, mushrooms, wheat).
    Cross,
    /// Fluid with a level-dependent surface height.
    Fluid,
    /// Small vertical stick attached to floor or wall.
    Torch,
    /// Thin quad on a wall.
    Ladder,
    /// Two-block tall door.
    Door,
    /// Half-height horizontal slab (bed).
    Bed,
    /// Flat wire on the floor.
    Wire,
    /// Thin plate on the floor.
    Plate,
    /// Small box on a wall/floor (button, lever).
    Button,
    /// Cactus: slightly inset cube.
    Cactus,
    /// Farmland: 15/16 high cube.
    Farmland,
    /// Piston head (arm + plate).
    PistonHead,
    /// Nothing rendered.
    None,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    None,
    Pickaxe,
    Axe,
    Shovel,
    Sword,
    Hoe,
}

#[derive(Clone, Copy, Debug)]
pub struct BlockProps {
    pub name: &'static str,
    pub shape: Shape,
    /// Blocks light and culls neighbour faces.
    pub opaque: bool,
    /// Has collision.
    pub solid: bool,
    /// Rendered in the alpha-blended pass.
    pub translucent: bool,
    /// Can be overwritten when placing a block (grass, water, snow layer).
    pub replaceable: bool,
    pub light: u8,
    /// Seconds to break by hand with no tool bonus. Negative = unbreakable.
    pub hardness: f32,
    pub tool: Tool,
    /// Minimum tool tier required to get a drop (0 wood/none, 1 stone, 2 iron, 3 diamond).
    pub min_tier: u8,
    /// How much light passes through (0 = fully, used for leaves/water attenuation).
    pub light_filter: u8,
    pub tint: u8,
}

pub const TINT_NONE: u8 = 0;
pub const TINT_GRASS: u8 = 1;
pub const TINT_FOLIAGE: u8 = 2;
pub const TINT_WATER: u8 = 3;

const fn cube(name: &'static str, hardness: f32, tool: Tool, tier: u8) -> BlockProps {
    BlockProps {
        name,
        shape: Shape::Cube,
        opaque: true,
        solid: true,
        translucent: false,
        replaceable: false,
        light: 0,
        hardness,
        tool,
        min_tier: tier,
        light_filter: 15,
        tint: TINT_NONE,
    }
}

const fn plant(name: &'static str) -> BlockProps {
    BlockProps {
        name,
        shape: Shape::Cross,
        opaque: false,
        solid: false,
        translucent: false,
        replaceable: false,
        light: 0,
        hardness: 0.0,
        tool: Tool::None,
        min_tier: 0,
        light_filter: 0,
        tint: TINT_NONE,
    }
}

const fn deco(name: &'static str, shape: Shape, hardness: f32, tool: Tool) -> BlockProps {
    BlockProps {
        name,
        shape,
        opaque: false,
        solid: false,
        translucent: false,
        replaceable: false,
        light: 0,
        hardness,
        tool,
        min_tier: 0,
        light_filter: 0,
        tint: TINT_NONE,
    }
}

fn build_props() -> Vec<BlockProps> {
    use Block::*;
    let mut v = vec![BlockProps {
        name: "air",
        shape: Shape::None,
        opaque: false,
        solid: false,
        translucent: false,
        replaceable: true,
        light: 0,
        hardness: 0.0,
        tool: Tool::None,
        min_tier: 0,
        light_filter: 0,
        tint: 0,
    }; 256];
    let p = |b: Block, props: BlockProps| (b.id() as usize, props);
    let list = [
        p(Stone, cube("stone", 1.5, Tool::Pickaxe, 0)),
        p(Grass, BlockProps { tint: TINT_GRASS, ..cube("grass block", 0.6, Tool::Shovel, 0) }),
        p(Dirt, cube("dirt", 0.5, Tool::Shovel, 0)),
        p(Cobblestone, cube("cobblestone", 2.0, Tool::Pickaxe, 0)),
        p(OakPlanks, cube("oak planks", 2.0, Tool::Axe, 0)),
        p(BirchPlanks, cube("birch planks", 2.0, Tool::Axe, 0)),
        p(SprucePlanks, cube("spruce planks", 2.0, Tool::Axe, 0)),
        p(Bedrock, cube("bedrock", -1.0, Tool::Pickaxe, 0)),
        p(Water, BlockProps {
            name: "water",
            shape: Shape::Fluid,
            opaque: false,
            solid: false,
            translucent: true,
            replaceable: true,
            light: 0,
            hardness: 100.0,
            tool: Tool::None,
            min_tier: 0,
            light_filter: 2,
            tint: TINT_WATER,
        }),
        p(Lava, BlockProps {
            name: "lava",
            shape: Shape::Fluid,
            opaque: false,
            solid: false,
            translucent: false,
            replaceable: true,
            light: 15,
            hardness: 100.0,
            tool: Tool::None,
            min_tier: 0,
            light_filter: 0,
            tint: TINT_NONE,
        }),
        p(Sand, cube("sand", 0.5, Tool::Shovel, 0)),
        p(Gravel, cube("gravel", 0.6, Tool::Shovel, 0)),
        p(CoalOre, cube("coal ore", 3.0, Tool::Pickaxe, 0)),
        p(IronOre, cube("iron ore", 3.0, Tool::Pickaxe, 1)),
        p(GoldOre, cube("gold ore", 3.0, Tool::Pickaxe, 2)),
        p(DiamondOre, cube("diamond ore", 3.0, Tool::Pickaxe, 2)),
        p(RedstoneOre, cube("redstone ore", 3.0, Tool::Pickaxe, 2)),
        p(OakLog, cube("oak log", 2.0, Tool::Axe, 0)),
        p(BirchLog, cube("birch log", 2.0, Tool::Axe, 0)),
        p(SpruceLog, cube("spruce log", 2.0, Tool::Axe, 0)),
        p(OakLeaves, BlockProps { opaque: false, light_filter: 1, tint: TINT_FOLIAGE, ..cube("oak leaves", 0.2, Tool::None, 0) }),
        p(BirchLeaves, BlockProps { opaque: false, light_filter: 1, ..cube("birch leaves", 0.2, Tool::None, 0) }),
        p(SpruceLeaves, BlockProps { opaque: false, light_filter: 1, ..cube("spruce leaves", 0.2, Tool::None, 0) }),
        p(Glass, BlockProps { opaque: false, translucent: true, light_filter: 0, ..cube("glass", 0.3, Tool::None, 0) }),
        p(Sandstone, cube("sandstone", 0.8, Tool::Pickaxe, 0)),
        p(Snow, cube("snow", 0.2, Tool::Shovel, 0)),
        p(Ice, BlockProps { opaque: false, translucent: true, light_filter: 2, ..cube("ice", 0.5, Tool::Pickaxe, 0) }),
        p(Cactus, BlockProps { shape: Shape::Cactus, opaque: false, ..cube("cactus", 0.4, Tool::None, 0) }),
        p(Clay, cube("clay", 0.6, Tool::Shovel, 0)),
        p(TallGrass, BlockProps { replaceable: true, tint: TINT_GRASS, ..plant("tall grass") }),
        p(DeadBush, BlockProps { replaceable: true, ..plant("dead bush") }),
        p(Dandelion, plant("dandelion")),
        p(Poppy, plant("poppy")),
        p(BrownMushroom, plant("brown mushroom")),
        p(RedMushroom, plant("red mushroom")),
        p(CraftingTable, cube("crafting table", 2.5, Tool::Axe, 0)),
        p(Furnace, cube("furnace", 3.5, Tool::Pickaxe, 0)),
        p(FurnaceLit, BlockProps { light: 13, ..cube("furnace", 3.5, Tool::Pickaxe, 0) }),
        p(Chest, BlockProps { opaque: false, ..cube("chest", 2.5, Tool::Axe, 0) }),
        p(Torch, BlockProps { light: 14, ..deco("torch", Shape::Torch, 0.0, Tool::None) }),
        p(Ladder, deco("ladder", Shape::Ladder, 0.4, Tool::Axe)),
        p(Door, BlockProps { solid: true, ..deco("oak door", Shape::Door, 3.0, Tool::Axe) }),
        p(Bed, BlockProps { solid: true, ..deco("bed", Shape::Bed, 0.2, Tool::None) }),
        p(RedstoneDust, deco("redstone dust", Shape::Wire, 0.0, Tool::None)),
        p(RedstoneTorchOn, BlockProps { light: 7, ..deco("redstone torch", Shape::Torch, 0.0, Tool::None) }),
        p(RedstoneTorchOff, deco("redstone torch", Shape::Torch, 0.0, Tool::None)),
        p(Lever, deco("lever", Shape::Button, 0.5, Tool::None)),
        p(Button, deco("stone button", Shape::Button, 0.5, Tool::None)),
        p(PressurePlate, deco("pressure plate", Shape::Plate, 0.5, Tool::None)),
        p(RedstoneLamp, cube("redstone lamp", 0.3, Tool::None, 0)),
        p(RedstoneLampLit, BlockProps { light: 15, ..cube("redstone lamp", 0.3, Tool::None, 0) }),
        p(Piston, BlockProps { opaque: false, ..cube("piston", 0.5, Tool::Pickaxe, 0) }),
        p(StickyPiston, BlockProps { opaque: false, ..cube("sticky piston", 0.5, Tool::Pickaxe, 0) }),
        p(PistonHead, BlockProps { shape: Shape::PistonHead, opaque: false, ..cube("piston head", 0.5, Tool::Pickaxe, 0) }),
        p(Tnt, cube("tnt", 0.0, Tool::None, 0)),
        p(Glowstone, BlockProps { light: 15, ..cube("glowstone", 0.3, Tool::Pickaxe, 0) }),
        p(MossyCobblestone, cube("mossy cobblestone", 2.0, Tool::Pickaxe, 0)),
        p(Spawner, BlockProps { opaque: false, ..cube("monster spawner", 5.0, Tool::Pickaxe, 0) }),
        p(Obsidian, cube("obsidian", 50.0, Tool::Pickaxe, 3)),
        p(Wool, cube("wool", 0.8, Tool::None, 0)),
        p(StoneBricks, cube("stone bricks", 1.5, Tool::Pickaxe, 0)),
        p(CrackedStoneBricks, cube("cracked stone bricks", 1.5, Tool::Pickaxe, 0)),
        p(IronBlock, cube("iron block", 5.0, Tool::Pickaxe, 1)),
        p(GoldBlock, cube("gold block", 3.0, Tool::Pickaxe, 2)),
        p(DiamondBlock, cube("diamond block", 5.0, Tool::Pickaxe, 2)),
        p(MushroomStem, cube("mushroom stem", 0.2, Tool::Axe, 0)),
        p(RedMushroomBlock, cube("red mushroom block", 0.2, Tool::Axe, 0)),
        p(BrownMushroomBlock, cube("brown mushroom block", 0.2, Tool::Axe, 0)),
        p(Farmland, BlockProps { shape: Shape::Farmland, opaque: false, ..cube("farmland", 0.6, Tool::Shovel, 0) }),
        p(Wheat, plant("wheat")),
        p(Podzol, cube("podzol", 0.5, Tool::Shovel, 0)),
        p(SnowyGrass, cube("snowy grass block", 0.6, Tool::Shovel, 0)),
        p(Bricks, cube("bricks", 2.0, Tool::Pickaxe, 0)),
        p(Bookshelf, cube("bookshelf", 1.5, Tool::Axe, 0)),
        p(HayBale, cube("hay bale", 0.5, Tool::None, 0)),
    ];
    for (i, props) in list {
        v[i] = props;
    }
    // non-opaque blocks that were derived from `cube` must not swallow light
    for p in v.iter_mut() {
        if !p.opaque && p.light_filter == 15 {
            p.light_filter = 0;
        }
    }
    v
}

static PROPS: OnceLock<Vec<BlockProps>> = OnceLock::new();

#[inline]
pub fn props(id: u8) -> &'static BlockProps {
    &PROPS.get_or_init(build_props)[id as usize]
}

#[inline]
pub fn props_of(b: Block) -> &'static BlockProps {
    props(b.id())
}

/// Packed voxel helpers.
#[inline]
pub const fn voxel(b: Block, meta: u8) -> u16 {
    (b as u16) | ((meta as u16) << 8)
}
#[inline]
pub fn vox_block(v: u16) -> Block {
    Block::from_id((v & 0xff) as u8)
}
#[inline]
pub const fn vox_id(v: u16) -> u8 {
    (v & 0xff) as u8
}
#[inline]
pub const fn vox_meta(v: u16) -> u8 {
    (v >> 8) as u8
}

#[inline]
pub fn is_opaque(v: u16) -> bool {
    props(vox_id(v)).opaque
}
#[inline]
pub fn is_solid(v: u16) -> bool {
    props(vox_id(v)).solid
}
#[inline]
pub fn is_air(v: u16) -> bool {
    vox_id(v) == 0
}
#[inline]
pub fn is_fluid(v: u16) -> bool {
    let id = vox_id(v);
    id == Block::Water.id() || id == Block::Lava.id()
}
#[inline]
pub fn is_water(v: u16) -> bool {
    vox_id(v) == Block::Water.id()
}
#[inline]
pub fn is_leaves(b: Block) -> bool {
    matches!(b, Block::OakLeaves | Block::BirchLeaves | Block::SpruceLeaves)
}
#[inline]
pub fn is_log(b: Block) -> bool {
    matches!(b, Block::OakLog | Block::BirchLog | Block::SpruceLog)
}

/// Faces: 0 = -X, 1 = +X, 2 = -Y, 3 = +Y, 4 = -Z, 5 = +Z.
pub const FACE_NORMALS: [[i32; 3]; 6] = [
    [-1, 0, 0],
    [1, 0, 0],
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
];

/// Tiles for each face of a cube-shaped block (west, east, bottom, top, north, south).
pub fn face_tiles(b: Block, meta: u8) -> [Tile; 6] {
    use Block::*;
    use Tile as T;
    let all = |t: Tile| [t; 6];
    let column = |side: Tile, end: Tile| [side, side, end, end, side, side];
    let topbot = |side: Tile, bottom: Tile, top: Tile| [side, side, bottom, top, side, side];
    let log = |side: Tile, end: Tile| match meta & 3 {
        1 => [end, end, side, side, side, side],
        2 => [side, side, side, side, end, end],
        _ => column(side, end),
    };
    // Front face by horizontal facing meta (0=-Z,1=+X,2=+Z,3=-X)
    let fronted = |side: Tile, front: Tile, top: Tile| {
        let mut f = [side; 6];
        f[2] = top;
        f[3] = top;
        let idx = match meta & 3 {
            0 => 4,
            1 => 1,
            2 => 5,
            _ => 0,
        };
        f[idx] = front;
        f
    };
    match b {
        Stone => all(T::Stone),
        Grass => topbot(T::GrassSide, T::Dirt, T::GrassTop),
        SnowyGrass => topbot(T::SnowGrassSide, T::Dirt, T::Snow),
        Dirt => all(T::Dirt),
        Cobblestone => all(T::Cobblestone),
        OakPlanks => all(T::OakPlanks),
        BirchPlanks => all(T::BirchPlanks),
        SprucePlanks => all(T::SprucePlanks),
        Bedrock => all(T::Bedrock),
        Water => all(T::Water),
        Lava => all(T::Lava),
        Sand => all(T::Sand),
        Gravel => all(T::Gravel),
        CoalOre => all(T::CoalOre),
        IronOre => all(T::IronOre),
        GoldOre => all(T::GoldOre),
        DiamondOre => all(T::DiamondOre),
        RedstoneOre => all(T::RedstoneOre),
        OakLog => log(T::OakLogSide, T::OakLogTop),
        BirchLog => log(T::BirchLogSide, T::BirchLogTop),
        SpruceLog => log(T::SpruceLogSide, T::SpruceLogTop),
        OakLeaves => all(T::OakLeaves),
        BirchLeaves => all(T::BirchLeaves),
        SpruceLeaves => all(T::SpruceLeaves),
        Glass => all(T::Glass),
        Sandstone => topbot(T::SandstoneSide, T::SandstoneTop, T::SandstoneTop),
        Snow => all(T::Snow),
        Ice => all(T::Ice),
        Cactus => topbot(T::CactusSide, T::CactusTop, T::CactusTop),
        Clay => all(T::Clay),
        TallGrass => all(T::TallGrass),
        DeadBush => all(T::DeadBush),
        Dandelion => all(T::Dandelion),
        Poppy => all(T::Poppy),
        BrownMushroom => all(T::BrownMushroom),
        RedMushroom => all(T::RedMushroom),
        CraftingTable => {
            let mut f = all(T::CraftingTableSide);
            f[3] = T::CraftingTableTop;
            f[2] = T::OakPlanks;
            f[4] = T::CraftingTableFront;
            f[5] = T::CraftingTableFront;
            f
        }
        Furnace => fronted(T::FurnaceSide, T::FurnaceFront, T::FurnaceTop),
        FurnaceLit => fronted(T::FurnaceSide, T::FurnaceFrontLit, T::FurnaceTop),
        Chest => fronted(T::ChestSide, T::ChestFront, T::ChestTop),
        Torch => all(T::Torch),
        Ladder => all(T::Ladder),
        Door => {
            if meta & 8 != 0 {
                all(T::DoorUpper)
            } else {
                all(T::DoorLower)
            }
        }
        Bed => all(T::BedTopHead),
        RedstoneDust => all(T::RedstoneDust),
        RedstoneTorchOn => all(T::RedstoneTorchOn),
        RedstoneTorchOff => all(T::RedstoneTorchOff),
        Lever => all(T::Lever),
        Button => all(T::Stone),
        PressurePlate => all(T::OakPlanks),
        RedstoneLamp => all(T::RedstoneLampOff),
        RedstoneLampLit => all(T::RedstoneLampOn),
        Piston | StickyPiston => {
            let face = if b == StickyPiston { T::PistonTopSticky } else { T::PistonTop };
            let front = if meta & 8 != 0 { T::PistonInner } else { face };
            let dir = (meta & 7) as usize;
            let mut f = all(T::PistonSide);
            let opposite = dir ^ 1;
            f[dir.min(5)] = front;
            f[opposite.min(5)] = T::PistonBottom;
            f
        }
        PistonHead => {
            let dir = (meta & 7) as usize;
            let mut f = all(T::PistonSide);
            f[dir.min(5)] = if meta & 8 != 0 { T::PistonTopSticky } else { T::PistonTop };
            f
        }
        Tnt => topbot(T::TntSide, T::TntBottom, T::TntTop),
        Glowstone => all(T::Glowstone),
        MossyCobblestone => all(T::MossyCobblestone),
        Spawner => all(T::Spawner),
        Obsidian => all(T::Obsidian),
        Wool => all(T::Wool),
        StoneBricks => all(T::StoneBricks),
        CrackedStoneBricks => all(T::CrackedStoneBricks),
        IronBlock => all(T::IronBlock),
        GoldBlock => all(T::GoldBlock),
        DiamondBlock => all(T::DiamondBlock),
        MushroomStem => all(T::MushroomStem),
        RedMushroomBlock => all(T::MushroomRed),
        BrownMushroomBlock => all(T::MushroomBrown),
        Farmland => topbot(T::Dirt, T::Dirt, T::Farmland),
        Wheat => all(match meta.min(7) {
            0 | 1 => T::Wheat0,
            2 | 3 => T::Wheat1,
            4 | 5 => T::Wheat2,
            6 => T::Wheat3,
            _ => T::Wheat4,
        }),
        Podzol => topbot(T::PodzolSide, T::Dirt, T::PodzolTop),
        Bricks => all(T::Bricks),
        Bookshelf => column(T::Bookshelf, T::OakPlanks),
        HayBale => column(T::HaySide, T::HayTop),
        Air => all(T::Stone),
    }
}

/// Convert a horizontal facing (0=-Z,1=+X,2=+Z,3=-X) to a unit offset.
pub fn facing_offset(f: u8) -> (i32, i32) {
    match f & 3 {
        0 => (0, -1),
        1 => (1, 0),
        2 => (0, 1),
        _ => (-1, 0),
    }
}

/// Facing of a yaw angle in radians (which way the *player looks*).
pub fn facing_from_yaw(yaw: f32) -> u8 {
    let (s, c) = yaw.sin_cos();
    // forward = (-sin(yaw), 0, -cos(yaw)) in our camera convention
    let fx = -s;
    let fz = -c;
    if fx.abs() > fz.abs() {
        if fx > 0.0 { 1 } else { 3 }
    } else if fz > 0.0 {
        2
    } else {
        0
    }
}

/// Face index (0..6) to direction index used by pistons: same numbering.
pub fn face_offset(f: u8) -> (i32, i32, i32) {
    let n = FACE_NORMALS[(f as usize).min(5)];
    (n[0], n[1], n[2])
}
