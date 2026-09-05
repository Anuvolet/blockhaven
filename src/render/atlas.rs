//! Texture atlas: a 2D array texture of 16x16 tiles, all generated procedurally.

use crate::render::texgen;

pub const TILE: usize = 16;

macro_rules! tiles {
    ($($name:ident),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[repr(u16)]
        pub enum Tile { $($name),* }
        impl Tile {
            pub const ALL: &'static [Tile] = &[$(Tile::$name),*];
            pub const COUNT: usize = Tile::ALL.len();
            #[inline]
            pub fn index(self) -> u16 { self as u16 }
        }
    };
}

tiles! {
    // --- blocks ---
    Stone, Dirt, GrassTop, GrassSide, SnowGrassSide, Cobblestone, MossyCobblestone,
    OakPlanks, BirchPlanks, SprucePlanks, Bedrock, Water, Lava, Sand, Gravel,
    CoalOre, IronOre, GoldOre, DiamondOre, RedstoneOre,
    OakLogSide, OakLogTop, BirchLogSide, BirchLogTop, SpruceLogSide, SpruceLogTop,
    OakLeaves, BirchLeaves, SpruceLeaves, Glass, SandstoneTop, SandstoneSide, Snow, Ice,
    CactusSide, CactusTop, Clay, TallGrass, DeadBush, Dandelion, Poppy, BrownMushroom,
    RedMushroom, CraftingTableTop, CraftingTableSide, CraftingTableFront, FurnaceFront,
    FurnaceFrontLit, FurnaceSide, FurnaceTop, ChestFront, ChestSide, ChestTop, Torch, Ladder,
    DoorLower, DoorUpper, BedTopHead, BedTopFoot, BedSide, BedEnd, RedstoneDust,
    RedstoneDustCross, RedstoneTorchOn, RedstoneTorchOff, Lever, RedstoneLampOff,
    RedstoneLampOn, PistonSide, PistonTop, PistonTopSticky, PistonBottom, PistonInner,
    TntSide, TntTop, TntBottom, Glowstone, Spawner, Obsidian, Wool, StoneBricks,
    CrackedStoneBricks, IronBlock, GoldBlock, DiamondBlock, MushroomStem, MushroomRed,
    MushroomBrown, Farmland, Wheat0, Wheat1, Wheat2, Wheat3, Wheat4, PodzolTop, PodzolSide,
    Bricks, Bookshelf, HaySide, HayTop,
    // --- items ---
    Stick, Coal, IronIngot, GoldIngot, Diamond, Redstone, Wheat, Bread, PorkchopRaw,
    PorkchopCooked, BeefRaw, BeefCooked, ChickenRaw, ChickenCooked, Leather, Feather, Arrow,
    Bone, RottenFlesh, Gunpowder, String, Apple, Egg, Bow, ClayBall, Brick, Flint,
    WoodPickaxe, WoodAxe, WoodShovel, WoodSword, WoodHoe,
    StonePickaxe, StoneAxe, StoneShovel, StoneSword, StoneHoe,
    IronPickaxe, IronAxe, IronShovel, IronSword, IronHoe,
    GoldPickaxe, GoldAxe, GoldShovel, GoldSword, GoldHoe,
    DiamondPickaxe, DiamondAxe, DiamondShovel, DiamondSword, DiamondHoe,
    LeatherHelmet, LeatherChest, LeatherLegs, LeatherBoots,
    IronHelmet, IronChest, IronLegs, IronBoots,
    DoorItem, BedItem, TorchItem, LadderItem,
    // --- mobs ---
    PigSkin, PigFace, CowSkin, CowFace, CowUdder, SheepWool, SheepFace, ChickenBody, ChickenFace,
    ChickenLeg, ZombieFace, ZombieShirt, ZombiePants, ZombieSkin, SkeletonFace, SkeletonBody,
    SkeletonLimb, CreeperFace, CreeperBody, PlayerFace, PlayerSkin, PlayerShirt, PlayerPants,
    ArrowSide, TntPrimed,
    // --- effects & ui ---
    Crack0, Crack1, Crack2, Crack3, Crack4, Crack5, Crack6, Crack7, Crack8, Crack9,
    HeartFull, HeartHalf, HeartEmpty, FoodFull, FoodHalf, FoodEmpty, ArmorFull, ArmorHalf,
    ArmorEmpty, Slot, SlotSelected, Sun, Moon, Bubble, White, Crosshair, ArrowUp, ArrowRight, Checkmark,
}

pub struct AtlasData {
    /// RGBA8, `TILE*TILE*4` bytes per layer, per mip level.
    pub mips: Vec<Vec<u8>>,
    pub layers: u32,
}

pub const MIP_LEVELS: u32 = 5; // 16, 8, 4, 2, 1

pub fn generate() -> AtlasData {
    let layers = Tile::COUNT as u32;
    let mut base = vec![0u8; TILE * TILE * 4 * layers as usize];
    for (i, t) in Tile::ALL.iter().enumerate() {
        let px = texgen::tile_pixels(*t);
        base[i * TILE * TILE * 4..(i + 1) * TILE * TILE * 4].copy_from_slice(&px);
    }
    let mut mips = vec![base];
    let mut size = TILE;
    for _ in 1..MIP_LEVELS {
        let prev = mips.last().unwrap();
        let ns = size / 2;
        let mut next = vec![0u8; ns * ns * 4 * layers as usize];
        for l in 0..layers as usize {
            for y in 0..ns {
                for x in 0..ns {
                    let mut acc = [0u32; 4];
                    let mut alpha_w = 0u32;
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let si = ((l * size + (y * 2 + dy)) * size + (x * 2 + dx)) * 4;
                            let a = prev[si + 3] as u32;
                            // weight colour by alpha so cut-outs don't bleed black
                            let w = a.max(1);
                            acc[0] += prev[si] as u32 * w;
                            acc[1] += prev[si + 1] as u32 * w;
                            acc[2] += prev[si + 2] as u32 * w;
                            acc[3] += a;
                            alpha_w += w;
                        }
                    }
                    let di = ((l * ns + y) * ns + x) * 4;
                    next[di] = (acc[0] / alpha_w) as u8;
                    next[di + 1] = (acc[1] / alpha_w) as u8;
                    next[di + 2] = (acc[2] / alpha_w) as u8;
                    next[di + 3] = (acc[3] / 4) as u8;
                }
            }
        }
        mips.push(next);
        size = ns;
    }
    AtlasData { mips, layers }
}

pub struct AtlasGpu {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub sampler_ui: wgpu::Sampler,
}

pub fn upload(device: &wgpu::Device, queue: &wgpu::Queue, data: &AtlasData) -> AtlasGpu {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("atlas"),
        size: wgpu::Extent3d {
            width: TILE as u32,
            height: TILE as u32,
            depth_or_array_layers: data.layers,
        },
        mip_level_count: MIP_LEVELS,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut size = TILE as u32;
    for (level, mip) in data.mips.iter().enumerate() {
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &tex,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            mip,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: data.layers,
            },
        );
        size /= 2;
    }
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("atlas sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        lod_min_clamp: 0.0,
        lod_max_clamp: 4.0,
        anisotropy_clamp: 1,
        ..Default::default()
    });
    let sampler_ui = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("ui sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        mipmap_filter: wgpu::FilterMode::Nearest,
        lod_min_clamp: 0.0,
        lod_max_clamp: 0.0,
        ..Default::default()
    });
    AtlasGpu { view, sampler, sampler_ui }
}
