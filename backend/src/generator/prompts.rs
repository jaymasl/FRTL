use std::fmt;
use rand::distributions::{Distribution, Standard};
use rand::Rng;
use strum::FromRepr;
use strum::EnumString;

#[derive(Debug, Clone, PartialEq, Eq, Hash, FromRepr, EnumString)]
#[strum(serialize_all = "PascalCase")]
#[repr(u8)]
pub enum Color {
    Rainbow, Gold, Silver, Black, White, 
    Purple, Green, Pink, Brown, Orange, 
    Red, Blue
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, FromRepr, EnumString)]
#[strum(serialize_all = "PascalCase")]
#[repr(u8)]
pub enum ArtStyle {
    Watercolor, Impressionism, Surrealism, Glass, Baroque, Gothic, Cubism,
    Abstract, Animated, Minimalist, Folk, Pixel, Graffiti, Anime, Pop,
    Sketch, Crayon, Doodle, Lowpoly, Papercraft, Plastic, Knit, Ceramic,
    Illusion, Retro, Plush, Metallic, Wooden
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, FromRepr, EnumString)]
#[strum(serialize_all = "PascalCase")]
#[repr(u8)]
pub enum EssenceType {
    Celestial, Ancient, Psychic, Undead, Fairy, Dark, Electric,
    Fire, Toxic, Construct, Air, Earth, Plant, Water, Fungal
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, FromRepr, EnumString)]
#[strum(serialize_all = "PascalCase")]
#[repr(u8)]
pub enum AnimalType {
    Dragon, Chimera, Alien,
    Unicorn, Lizard, Kraken, Penguin, Megalodon,
    Mammoth, Tyrannosaurus, Pangolin, Bee, Whale, Squid,
    Axolotl, Chameleon, Jellyfish, Mantis, Scorpion,
    Peacock, Parrot, Eagle, Owl, Crow, Duck, Chicken, 
    Crocodile, Turtle, Tiger, Wolf, Lion, Jaguar, Fox, Dog, Cat, Rhinoceros, 
    Bear, Deer, Dolphin, Elephant, Crab, Raccoon, Sheep, Goat, Pig, Mouse, 
    Hamster, Rabbit, Squirrel, Rat, Frog, Otter,
    Horse, Donkey, Turkey, Goose, Llama, Bison, Giraffe, Zebra, Panda, 
    Kangaroo, Koala, Flamingo, Cow, Spider, Sloth, Toucan
}

impl Color {
    const fn variant_count() -> usize { 13 }

    pub const fn description(&self) -> &'static str {
        match self {
            Self::Rainbow => "Rainbow glitter shine", Self::Gold => "Gold yellow glitter shine", 
            Self::Silver => "Silver gray glitter shine", Self::Black => "Black", 
            Self::White => "White", Self::Purple => "Purple", Self::Green => "Green", 
            Self::Pink => "Pink", Self::Brown => "Brown", Self::Orange => "Orange", 
            Self::Red => "Red", Self::Blue => "Blue"
        }
    }
}

impl ArtStyle {
    const fn variant_count() -> usize { 28 }

    pub const fn description(&self) -> &'static str {
        match self {
            Self::Watercolor => "Watercolor art style with fluid edges, translucent colors, visible brush strokes",
            Self::Impressionism => "Impressionist art style with painted edges, thick palette knife paint, emphasis on light, atmosphere",
            Self::Surrealism => "Surrealist art style with painted imagery, weird odd, surreal",
            Self::Glass => "Stained glass style with thick outlines, geometric segments, jewel-toned colors",
            Self::Baroque => "Baroque art style with ornate details, dramatic lighting, rich textures",
            Self::Gothic => "Gothic art style with dark palettes, dramatic contrasts, ornate patterns",
            Self::Cubism => "Cubist art style with blocky fragmented shapes, bold geometric lines, muted earthy colors",
            Self::Abstract => "Abstract art style with strange shaped forms, abstractionism, splatter dynamic compositions",
            Self::Animated => "3D animated art style with rounded shapes, vivid colors, dynamic lighting, expressive designs",
            Self::Minimalist => "Minimalist art style with simple textures, bold clean lines, visible outlines",
            Self::Folk => "Folk art style with flat patterns, bold colors, traditional symmetrical motifs",
            Self::Pixel => "Pixel art style with blocky pixels, high quality retro aesthetics",
            Self::Graffiti => "Graffiti art style with bold style, urban wall, spray paint",
            Self::Anime => "Anime art style with sharp outlines, bold shading, dynamic compositions",
            Self::Pop => "Pop art style with striking outlines, dotted dots, cartoon comic book vibrant flat colors, dynamic bold patterns",
            Self::Sketch => "Sketch art style with fine lines, intricate shading, amateur drawing, scribble",
            Self::Crayon => "Crayon drawn art style with uneven strokes, vibrant hues, bad artist, spontaneous look",
            Self::Doodle => "Doodle art style with playful, freeform lines, simple shapes, whimsical creativity",
            Self::Lowpoly => "Lowpoly art style with simplified geometric shapes, block faceted appearance",
            Self::Papercraft => "Papercraft style with cut shapes, layered structures, diverse designs, intricate patterns",
            Self::Plastic => "Plastic block toy style with modular, interlocking brick shapes, playful bright colors",
            Self::Knit => "Knit yarn style with textured fiber patterns, handcrafted crochet amigurumi",
            Self::Ceramic => "Ceramic art style with smooth surfaces, earthy tones, intricate glazes, and sculptural forms",
            Self::Illusion => "Optical illusion art style with mesmerizing mirror optical patterns, strange weird perspective",
            Self::Retro => "Retro art style with lomograph quality aesthetic, strong chromatic abberation, vibrant saturation, warm vintage",
            Self::Plush => "Plush toy style with soft fuzzy textures, vibrant colors, whimsical cozy",
            Self::Metallic => "Metallic hammered art style with textured surfaces, reflective sheen, embossed patterns, industrial stamped elegance",
            Self::Wooden => "Wooden art style with natural grain patterns, wood texture carved carving, organic shapes, handcrafted"
        }
    }
}

impl EssenceType {
    const fn variant_count() -> usize { 15 }

    pub const fn description(&self) -> &'static str {
        match self {
            Self::Celestial => "Celestial cosmic stellar astral divine day luminous radiant magical",
            Self::Ancient => "Ancient old parchment papyrus primordial fossil timeless eternal archaic weathered",
            Self::Psychic => "Psychic mental rays telepathic signal beams radiating spiritual transcendent",
            Self::Undead => "Undead decay lifeless ghost haunting spectral withered necrotic stitches spooky",
            Self::Fairy => "Sparkling glitter radiant enchanted whimsical playful fun",
            Self::Dark => "Dark shadow umbral tenebrous night stygian shade obsidian pitch-black",
            Self::Electric => "Electric lightning thunder plasma static charged volt voltage voltaic",
            Self::Fire => "Fire flame inferno blaze burning lava flickering smoldering charred",
            Self::Toxic => "Toxic drip venomous poisonous noxious corrosive acrid lethal",
            Self::Construct => "Constructed mechanical artificial synthetic cog engineered robotic cybernetic",
            Self::Air => "Air flowing wind windy flow breeze gust tempest zephyr swirling",
            Self::Earth => "Ground stone crystal crystalline diamond mineral soil sediment pebble dirt",
            Self::Plant => "Plant jungle floral verdant natural organic lush thriving",
            Self::Water => "Aquatic wet liquid fluid oceanic rippling flowing water crystalline tranquil",
            Self::Fungal => "Fungal fungus fungi mycelium spore organic earthy symbiotic"
        }
    }
}

impl AnimalType {
    const fn variant_count() -> usize { 68 }
    
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Dragon => "Dragon", Self::Alien => "Alien", Self::Chimera => "Chimera",
            Self::Cow => "Cow", Self::Unicorn => "Unicorn", Self::Lizard => "Lizard",
            Self::Kraken => "Kraken", Self::Megalodon => "Megalodon",
            Self::Penguin => "Penguin", Self::Mammoth => "Mammoth",
            Self::Tyrannosaurus => "Tyrannosaurus", Self::Pangolin => "Pangolin", Self::Bee => "Bee",
            Self::Whale => "Whale", Self::Squid => "Squid", Self::Axolotl => "Axolotl",
            Self::Chameleon => "Chameleon", Self::Jellyfish => "Jellyfish", Self::Mantis => "Mantis",
            Self::Scorpion => "Scorpion", Self::Peacock => "Peacock", Self::Parrot => "Parrot",
            Self::Eagle => "Eagle", Self::Owl => "Owl",
            Self::Crow => "Crow", Self::Duck => "Duck", Self::Chicken => "Chicken",
            Self::Crocodile => "Crocodile", Self::Turtle => "Turtle",
            Self::Tiger => "Tiger", Self::Wolf => "Wolf", Self::Lion => "Lion",
            Self::Jaguar => "Jaguar", Self::Fox => "Fox", Self::Dog => "Dog",
            Self::Cat => "Cat", Self::Rhinoceros => "Rhinoceros", Self::Bear => "Bear",
            Self::Deer => "Deer", Self::Dolphin => "Dolphin", Self::Elephant => "Elephant",
            Self::Crab => "Crab", Self::Raccoon => "Raccoon", Self::Sheep => "Sheep",
            Self::Goat => "Goat", Self::Pig => "Pig", Self::Mouse => "Mouse",
            Self::Hamster => "Hamster", Self::Rabbit => "Rabbit", Self::Squirrel => "Squirrel",
            Self::Rat => "Rat", Self::Frog => "Frog", Self::Otter => "Otter",
            Self::Horse => "Horse", Self::Donkey => "Donkey", Self::Turkey => "Turkey",
            Self::Goose => "Goose", Self::Llama => "Llama", Self::Bison => "Bison",
            Self::Giraffe => "Giraffe", Self::Zebra => "Zebra", Self::Panda => "Panda",
            Self::Kangaroo => "Kangaroo", Self::Koala => "Koala", Self::Flamingo => "Flamingo",
            Self::Spider => "Spider", Self::Sloth => "Sloth", Self::Toucan => "Toucan"
        }              
    }
}

macro_rules! impl_display {
    ($type:ty) => {
        impl fmt::Display for $type {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{:?}", self)
            }
        }
    }
}

impl_display!(Color);
impl_display!(ArtStyle);
impl_display!(EssenceType);
impl_display!(AnimalType);

macro_rules! impl_distribution {
    ($type:ty) => {
        impl Distribution<$type> for Standard {
            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> $type {
                let variant_count = <$type>::variant_count();
                match <$type>::from_repr(rng.gen_range(0..variant_count) as u8) {
                    Some(variant) => variant,
                    None => <$type>::from_repr(0).unwrap_or_else(|| panic!("Failed to generate random variant for {:?}", stringify!($type)))
                }
            }
        }
    }
}

impl_distribution!(Color);
impl_distribution!(ArtStyle);
impl_distribution!(EssenceType);
impl_distribution!(AnimalType);