use super::DecryptedShard;
use crate::errors::{MpcError, Result};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

/// Backup shard (Shard 3) — offline cold storage held by the user.
///
/// Three backup strategies:
/// 1. Social Recovery: 3-of-5 Shamir split among trusted contacts
/// 2. Mnemonic Phrase: BIP-39-style words encoding the shard
/// 3. Hardware Key: stored on YubiKey / FIDO2 device

/// A Shamir share distributed to a trusted contact.
#[derive(Clone, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
pub struct ShamirShare {
    pub index: u8,
    pub threshold: u8,
    pub total: u8,
    pub data: Vec<u8>,
}

impl ShamirShare {
    /// Create a new Shamir share.
    pub fn new(index: u8, threshold: u8, total: u8, data: Vec<u8>) -> Self {
        Self {
            index,
            threshold,
            total,
            data,
        }
    }

    /// Get the share's x-coordinate (index) for interpolation.
    pub fn x(&self) -> u8 {
        self.index
    }

    /// Get the share's y-value (secret data).
    pub fn y(&self) -> &[u8] {
        &self.data
    }
}

/// GF(256) arithmetic using AES irreducible polynomial: x^8 + x^4 + x^3 + x + 1
mod gf256 {
    /// Add two elements in GF(256) (XOR).
    #[inline(always)]
    pub fn add(a: u8, b: u8) -> u8 {
        a ^ b
    }

    /// Multiply two elements in GF(256).
    #[inline(always)]
    pub fn mul(a: u8, b: u8) -> u8 {
        let mut result = 0u8;
        let mut a = a;
        let mut b = b;
        for _ in 0..8 {
            if b & 1 != 0 {
                result ^= a;
            }
            let hi_bit = a & 0x80;
            a <<= 1;
            if hi_bit != 0 {
                a ^= 0x1b; // AES irreducible polynomial
            }
            b >>= 1;
        }
        result
    }

    /// Compute the multiplicative inverse using Fermat's little theorem.
    #[inline(always)]
    pub fn inverse(x: u8) -> u8 {
        assert!(x != 0, "inverse of zero");
        // x^254 mod polynomial in GF(256)
        let mut result = 1u8;
        let mut base = x;
        let mut exp = 254u16;
        while exp > 0 {
            if exp & 1 != 0 {
                result = mul(result, base);
            }
            base = mul(base, base);
            exp >>= 1;
        }
        result
    }

    /// Evaluate polynomial at x using Horner's method in GF(256).
    pub fn eval_poly(coeffs: &[u8], x: u8) -> u8 {
        let mut result = 0u8;
        for &coeff in coeffs.iter().rev() {
            result = mul(result, x);
            result = add(result, coeff);
        }
        result
    }
}

/// Split a backup shard into Shamir shares for social recovery.
///
/// Uses Shamir's Secret Sharing over GF(256) to split the shard into
/// `total` shares, of which any `threshold` can reconstruct the original.
pub fn split_for_social_recovery(
    shard: &DecryptedShard,
    threshold: u8,
    total: u8,
) -> Result<Vec<ShamirShare>> {
    if threshold < 2 {
        return Err(MpcError::ShardEncryption(
            "threshold must be at least 2".into(),
        ));
    }
    if threshold > total {
        return Err(MpcError::ShardEncryption(format!(
            "invalid threshold: {threshold}-of-{total}"
        )));
    }
    if total > 255 {
        return Err(MpcError::ShardEncryption(
            "too many shares (max 255)".into(),
        ));
    }

    let secret = shard.as_bytes();
    let secret_len = secret.len();

    // For each byte position, create a polynomial and evaluate
    let mut rng = rand::thread_rng();
    let mut shares = Vec::with_capacity(total as usize);

    // Initialize shares
    for i in 0..total {
        shares.push(ShamirShare {
            index: i + 1, // x-coordinates start at 1
            threshold,
            total,
            data: vec![0u8; secret_len],
        });
    }

    // For each byte in the secret
    for (byte_idx, &secret_byte) in secret.iter().enumerate() {
        // Create random polynomial coefficients: f(0) = secret_byte
        let mut coeffs = vec![0u8; threshold as usize];
        coeffs[0] = secret_byte; // f(0) = secret
        for coeff in coeffs.iter_mut().skip(1) {
            *coeff = rng.next_u32() as u8;
        }

        // Evaluate polynomial at each share's x-coordinate
        for share in &mut shares {
            let y = gf256::eval_poly(&coeffs, share.index);
            share.data[byte_idx] = y;
        }

        // Zeroize coefficients
        for c in coeffs.iter_mut() {
            c.zeroize();
        }
    }

    Ok(shares)
}

/// Reconstruct a backup shard from Shamir shares using Lagrange interpolation.
pub fn reconstruct_from_shares(shares: &[ShamirShare]) -> Result<DecryptedShard> {
    if shares.is_empty() {
        return Err(MpcError::ShardDecryption("no shares provided".into()));
    }

    let threshold = shares[0].threshold;
    if shares.len() < threshold as usize {
        return Err(MpcError::InsufficientParties {
            required: threshold as u16,
            available: shares.len() as u16,
        });
    }

    // Verify all shares have the same threshold and total
    for share in shares {
        if share.threshold != threshold {
            return Err(MpcError::ShardDecryption(
                "mismatched threshold in shares".into(),
            ));
        }
    }

    let secret_len = shares[0].data.len();
    let mut secret = vec![0u8; secret_len];

    // Take exactly threshold shares for reconstruction
    let shares = &shares[0..threshold as usize];

    // For each byte position, perform Lagrange interpolation
    for byte_idx in 0..secret_len {
        // Collect (x, y) points for this byte
        let points: Vec<(u8, u8)> = shares
            .iter()
            .map(|s| (s.x(), s.y()[byte_idx]))
            .collect();

        // Lagrange interpolation at x=0
        let mut result = 0u8;
        for (i, &(xi, yi)) in points.iter().enumerate() {
            // Compute Lagrange basis polynomial L_i(0)
            let mut lagrange = 1u8;
            for (j, &(xj, _)) in points.iter().enumerate() {
                if i != j {
                    // L_i(0) = product over j != i of (-xj) / (xi - xj)
                    let num = gf256::add(0, xj); // -xj in GF(256) = xj (since -1=1 in char 2)
                    let den = gf256::add(xi, xj); // xi - xj in GF(256) = xi + xj
                    let inv_den = gf256::inverse(den);
                    let term = gf256::mul(num, inv_den);
                    lagrange = gf256::mul(lagrange, term);
                }
            }
            result = gf256::add(result, gf256::mul(yi, lagrange));
        }

        secret[byte_idx] = result;
    }

    Ok(DecryptedShard::from_bytes(secret))
}

// BIP-39 word list (English)
const BIP39_WORDS: &[&str] = &[
    "abandon", "ability", "able", "about", "above", "absent", "absorb", "abstract", "absurd",
    "abuse", "access", "accident", "account", "accuse", "achieve", "acid", "acoustic", "acquire",
    "across", "act", "action", "actor", "actress", "actual", "adapt", "add", "addict",
    "address", "adjust", "admit", "adult", "advance", "advice", "aerobic", "affair", "afford",
    "afraid", "again", "age", "agent", "agree", "ahead", "aim", "air", "airport",
    "aisle", "alarm", "album", "alcohol", "alert", "alien", "all", "alley", "allow",
    "almost", "alone", "alpha", "already", "also", "alter", "always", "amateur", "amazing",
    "among", "amount", "amused", "analyst", "anchor", "ancient", "anger", "angle", "angry",
    "animal", "ankle", "announce", "annual", "another", "answer", "antenna", "antique", "anxiety",
    "any", "apart", "apology", "appear", "apple", "approve", "april", "arch", "arctic",
    "area", "arena", "argue", "arm", "armed", "armor", "army", "around", "arrange",
    "arrest", "arrive", "arrow", "art", "artefact", "artist", "artwork", "ask", "aspect",
    "assault", "asset", "assist", "assume", "asthma", "athlete", "atom", "attack", "attend",
    "attitude", "attract", "auction", "audit", "august", "aunt", "author", "auto", "autumn",
    "average", "avocado", "avoid", "awake", "aware", "away", "awesome", "awful", "awkward",
    "axis", "baby", "bachelor", "bacon", "badge", "bag", "balance", "balcony", "ball",
    "bamboo", "banana", "banner", "bar", "barely", "bargain", "barrel", "base", "basic",
    "basket", "battle", "beach", "bean", "beauty", "because", "become", "beef", "before",
    "begin", "behave", "behind", "believe", "below", "belt", "bench", "benefit", "best",
    "betray", "better", "between", "beyond", "bicycle", "bid", "bike", "bind", "biology",
    "bird", "birth", "bitter", "black", "blade", "blame", "blanket", "blast", "bleak",
    "bless", "blind", "blood", "blossom", "blouse", "blue", "blur", "blush", "board",
    "boat", "body", "boil", "bomb", "bone", "bonus", "book", "boost", "border",
    "boring", "borrow", "boss", "bottom", "bounce", "box", "boy", "bracket", "brain",
    "brand", "brass", "brave", "bread", "breeze", "brick", "bridge", "brief", "bright",
    "bring", "brisk", "broccoli", "broken", "bronze", "broom", "brother", "brown", "brush",
    "bubble", "buddy", "budget", "buffalo", "build", "bulb", "bulk", "bullet", "bundle",
    "bunker", "burden", "burger", "burst", "bus", "business", "busy", "butter", "buyer",
    "buzz", "cabbage", "cabin", "cable", "cactus", "cage", "cake", "call", "calm",
    "camera", "camp", "can", "canal", "cancel", "candy", "cannon", "canoe", "canvas",
    "canyon", "capable", "capital", "captain", "car", "carbon", "card", "cargo", "carpet",
    "carry", "cart", "case", "cash", "casino", "castle", "casual", "cat", "catalog",
    "catch", "category", "cattle", "caught", "cause", "caution", "cave", "ceiling", "celery",
    "cement", "census", "century", "cereal", "certain", "chair", "chalk", "champion", "change",
    "chaos", "chapter", "charge", "chase", "chat", "cheap", "check", "cheese", "chef",
    "cherry", "chest", "chicken", "chief", "child", "chimney", "choice", "choose", "chronic",
    "chuckle", "chunk", "churn", "cigar", "cinnamon", "circle", "citizen", "city", "civil",
    "claim", "clap", "clarify", "claw", "clay", "clean", "clerk", "clever", "click",
    "client", "cliff", "climb", "clinic", "clip", "clock", "clog", "close", "cloth",
    "cloud", "clown", "club", "clump", "cluster", "clutch", "coach", "coast", "coconut",
    "code", "coffee", "coil", "coin", "collect", "color", "column", "combine", "come",
    "comfort", "comic", "common", "company", "concert", "conduct", "confirm", "congress", "connect",
    "consider", "control", "convince", "cook", "cool", "copper", "copy", "coral", "core",
    "corn", "correct", "cost", "cotton", "couch", "country", "couple", "course", "cousin",
    "cover", "coyote", "crack", "cradle", "craft", "cram", "crane", "crash", "crater",
    "crawl", "crazy", "cream", "credit", "creek", "crew", "cricket", "crime", "crisp",
    "critic", "crop", "cross", "crouch", "crowd", "crucial", "cruel", "cruise", "crumble",
    "crunch", "crush", "cry", "crystal", "cube", "culture", "cup", "cupboard", "curious",
    "current", "curtain", "curve", "cushion", "custom", "cute", "cycle", "dad", "damage",
    "damp", "dance", "danger", "daring", "dash", "daughter", "dawn", "day", "deal",
    "debate", "debris", "decade", "december", "decide", "decline", "decorate", "decrease", "deer",
    "defense", "define", "defy", "degree", "delay", "deliver", "demand", "demise", "denial",
    "dentist", "deny", "depart", "depend", "deposit", "depth", "deputy", "derive", "describe",
    "desert", "design", "desk", "despair", "destroy", "detail", "detect", "develop", "device",
    "devote", "diagram", "dial", "diamond", "diary", "dice", "diesel", "diet", "differ",
    "digital", "dignity", "dilemma", "dinner", "dinosaur", "direct", "dirt", "disagree", "discover",
    "disease", "dish", "dismiss", "disorder", "display", "distance", "divert", "divide", "divorce",
    "dizzy", "doctor", "document", "dog", "doll", "dolphin", "domain", "donate", "donkey",
    "donor", "door", "dose", "double", "dove", "draft", "dragon", "drama", "drastic",
    "draw", "dream", "dress", "drift", "drill", "drink", "drip", "drive", "drop",
    "drum", "dry", "duck", "dumb", "dune", "during", "dust", "dutch", "duty",
    "dwarf", "dynamic", "eager", "eagle", "early", "earn", "earth", "easily", "east",
    "easy", "echo", "ecology", "economy", "edge", "edit", "educate", "effort", "egg",
    "eight", "either", "elbow", "elder", "electric", "elegant", "element", "elephant", "elevator",
    "elite", "else", "embark", "embody", "embrace", "emerge", "emotion", "employ", "empower",
    "empty", "enable", "enact", "end", "endless", "endorse", "enemy", "energy", "enforce",
    "engage", "engine", "enhance", "enjoy", "enlist", "enough", "enrich", "enroll", "ensure",
    "enter", "entire", "entry", "envelope", "episode", "equal", "equip", "era", "erase",
    "erode", "erosion", "error", "erupt", "escape", "essay", "essence", "estate", "eternal",
    "ethics", "evidence", "evil", "evoke", "evolve", "exact", "example", "excess", "exchange",
    "excite", "exclude", "excuse", "execute", "exercise", "exhaust", "exhibit", "exile", "exist",
    "exit", "exotic", "expand", "expect", "expire", "explain", "expose", "express", "extend",
    "extra", "eye", "eyebrow", "fabric", "face", "faculty", "fade", "faint", "faith",
    "fall", "false", "fame", "family", "famous", "fan", "fancy", "fantasy", "farm",
    "fashion", "fat", "fatal", "father", "fatigue", "fault", "favorite", "feature", "february",
    "federal", "fee", "feed", "feel", "female", "fence", "festival", "fetch", "fever",
    "few", "fiber", "fiction", "field", "figure", "file", "film", "filter", "final",
    "find", "fine", "finger", "finish", "fire", "firm", "first", "fiscal", "fish",
    "fit", "fitness", "fix", "flag", "flame", "flash", "flat", "flavor", "flee",
    "flight", "flip", "float", "flock", "floor", "flower", "fluid", "flush", "fly",
    "foam", "focus", "fog", "foil", "fold", "follow", "food", "foot", "force",
    "forest", "forget", "fork", "fortune", "forum", "forward", "fossil", "foster", "found",
    "fox", "fragile", "frame", "frequent", "fresh", "friend", "fringe", "frog", "front",
    "frost", "frown", "frozen", "fruit", "fuel", "fun", "funny", "furnace", "fury",
    "future", "gadget", "gain", "galaxy", "gallery", "game", "gap", "garage", "garbage",
    "garden", "garlic", "garment", "gas", "gasp", "gate", "gather", "gauge", "gaze",
    "general", "genius", "genre", "gentle", "genuine", "gesture", "ghost", "giant", "gift",
    "giggle", "ginger", "giraffe", "girl", "give", "glad", "glance", "glare", "glass",
    "glide", "glimpse", "globe", "gloom", "glory", "glove", "glow", "glue", "goat",
    "goddess", "gold", "good", "goose", "gorilla", "gospel", "gossip", "govern", "gown",
    "grab", "grace", "grain", "grant", "grape", "grass", "gravity", "great", "green",
    "grid", "grief", "grit", "grocery", "group", "grow", "grunt", "guard", "guess",
    "guide", "guilt", "guitar", "gun", "gym", "habit", "hair", "half", "hammer",
    "hamster", "hand", "happy", "harbor", "hard", "harsh", "harvest", "hat", "have",
    "hawk", "hazard", "head", "health", "heart", "heavy", "hedgehog", "height", "hello",
    "helmet", "help", "hen", "hero", "hidden", "high", "hill", "hint", "hip",
    "hire", "history", "hobby", "hockey", "hold", "hole", "holiday", "hollow", "home",
    "honey", "hood", "hope", "horn", "horror", "horse", "hospital", "host", "hotel",
    "hour", "hover", "hub", "huge", "human", "humble", "humor", "hundred", "hungry",
    "hunt", "hurdle", "hurry", "hurt", "husband", "hybrid", "ice", "icon", "idea",
    "identify", "idle", "ignore", "ill", "illegal", "illness", "image", "imitate", "immense",
    "immune", "impact", "impose", "improve", "impulse", "inch", "include", "income", "increase",
    "index", "indicate", "indoor", "industry", "infant", "inflict", "inform", "inhale", "inherit",
    "initial", "inject", "injury", "inmate", "inner", "innocent", "input", "inquiry", "insane",
    "insect", "inside", "inspire", "install", "intact", "interest", "into", "invest", "invite",
    "involve", "iron", "island", "isolate", "issue", "item", "ivory", "jacket", "jaguar",
    "jar", "jazz", "jealous", "jeans", "jelly", "jewel", "job", "join", "joke",
    "journey", "joy", "judge", "juice", "jump", "jungle", "junior", "junk", "just",
    "kangaroo", "keen", "keep", "ketchup", "key", "kick", "kid", "kidney", "kind",
    "kingdom", "kiss", "kit", "kitchen", "kite", "kitten", "kiwi", "knee", "knife",
    "knock", "know", "lab", "label", "labor", "ladder", "lady", "lake", "lamp",
    "language", "laptop", "large", "later", "latin", "laugh", "laundry", "lava", "law",
    "lawn", "lawsuit", "layer", "lazy", "leader", "leaf", "learn", "leave", "lecture",
    "left", "leg", "legal", "legend", "leisure", "lemon", "lend", "length", "lens",
    "leopard", "lesson", "letter", "level", "liar", "liberty", "library", "license", "life",
    "lift", "light", "like", "limb", "limit", "link", "lion", "liquid", "list",
    "little", "live", "lizard", "load", "loan", "lobster", "local", "lock", "logic",
    "lonely", "long", "loop", "lottery", "loud", "lounge", "love", "loyal", "lucky",
    "luggage", "lumber", "lunar", "lunch", "luxury", "lyrics", "machine", "mad", "magic",
    "magnet", "maid", "mail", "main", "major", "make", "mammal", "man", "manage",
    "mandate", "mango", "mansion", "manual", "maple", "marble", "march", "margin", "marine",
    "market", "marriage", "mask", "mass", "master", "match", "material", "math", "matrix",
    "matter", "maximum", "maze", "meadow", "mean", "measure", "meat", "mechanic", "medal",
    "media", "melody", "melt", "member", "memory", "mention", "menu", "mercy", "merge",
    "merit", "merry", "mesh", "message", "metal", "method", "middle", "midnight", "milk",
    "million", "mimic", "mind", "minimum", "minor", "minute", "miracle", "mirror", "misery",
    "miss", "mistake", "mix", "mixed", "mixture", "mobile", "model", "modify", "mom",
    "moment", "monitor", "monkey", "monster", "month", "moon", "moral", "more", "morning",
    "mosquito", "mother", "motion", "motor", "mountain", "mouse", "move", "movie", "much",
    "muffin", "mule", "multiply", "muscle", "museum", "mushroom", "music", "must", "mutual",
    "myself", "mystery", "myth", "naive", "name", "napkin", "narrow", "nasty", "nation",
    "nature", "near", "neck", "need", "negative", "neglect", "neither", "nephew", "nerve",
    "nest", "net", "network", "neutral", "never", "news", "next", "nice", "night",
    "noble", "noise", "nominee", "noodle", "normal", "north", "nose", "notable", "note",
    "nothing", "notice", "novel", "now", "nuclear", "number", "nurse", "nut", "oak",
    "obey", "object", "oblige", "obscure", "observe", "obtain", "obvious", "occur", "ocean",
    "october", "odor", "off", "offer", "office", "often", "oil", "okay", "old",
    "olive", "olympic", "omit", "once", "one", "onion", "online", "only", "open",
    "opera", "opinion", "oppose", "option", "orange", "orbit", "orchard", "order", "ordinary",
    "organ", "orient", "original", "orphan", "ostrich", "other", "outdoor", "outer", "output",
    "outside", "oval", "oven", "over", "own", "owner", "oxygen", "oyster", "ozone",
    "pact", "paddle", "page", "pair", "palace", "palm", "panda", "panel", "panic",
    "panther", "paper", "parade", "parent", "park", "parrot", "party", "pass", "patch",
    "path", "patient", "patrol", "pattern", "pause", "pave", "payment", "peace", "peanut",
    "pear", "peasant", "pelican", "pen", "penalty", "pencil", "people", "pepper", "perfect",
    "permit", "person", "pet", "phone", "photo", "phrase", "physical", "piano", "picnic",
    "picture", "piece", "pig", "pigeon", "pill", "pilot", "pink", "pioneer", "pipe",
    "pistol", "pitch", "pizza", "place", "planet", "plastic", "plate", "play", "please",
    "pledge", "pluck", "plug", "plunge", "poem", "poet", "point", "polar", "pole",
    "police", "pond", "pony", "pool", "popular", "portion", "position", "possible", "post",
    "potato", "pottery", "poverty", "powder", "power", "practice", "praise", "predict", "prefer",
    "prepare", "present", "pretty", "prevent", "price", "pride", "primary", "print", "priority",
    "prison", "private", "prize", "problem", "process", "produce", "profit", "program", "project",
    "promote", "proof", "property", "prosper", "protect", "proud", "provide", "public", "pudding",
    "pull", "pulp", "pulse", "pumpkin", "punch", "pupil", "puppy", "purchase", "purity",
    "purpose", "purse", "push", "put", "puzzle", "pyramid", "quality", "quantum", "quarter",
    "question", "quick", "quit", "quiz", "quote", "rabbit", "raccoon", "race", "rack",
    "radar", "radio", "rail", "rain", "raise", "rally", "ramp", "ranch", "random",
    "range", "rapid", "rare", "rate", "rather", "raven", "raw", "razor", "ready",
    "real", "reason", "rebel", "rebuild", "recall", "receive", "recipe", "record", "recycle",
    "reduce", "reflect", "reform", "refuse", "region", "regret", "regular", "reject", "relax",
    "release", "relief", "rely", "remain", "remember", "remind", "remove", "render", "renew",
    "rent", "reopen", "repair", "repeat", "replace", "report", "require", "rescue", "resemble",
    "resist", "resource", "response", "result", "retire", "retreat", "return", "reunion", "reveal",
    "review", "reward", "rhythm", "rib", "ribbon", "rice", "rich", "ride", "ridge",
    "rifle", "right", "rigid", "ring", "riot", "ripple", "risk", "ritual", "rival",
    "river", "road", "roast", "robot", "robust", "rocket", "romance", "roof", "rookie",
    "room", "rose", "rotate", "rough", "round", "route", "royal", "rubber", "rude",
    "rug", "rule", "run", "runway", "rural", "sad", "saddle", "sadness", "safe",
    "sail", "salad", "salmon", "salon", "salt", "salute", "same", "sample", "sand",
    "satisfy", "satoshi", "sauce", "sausage", "save", "say", "scale", "scan", "scare",
    "scatter", "scene", "scheme", "school", "science", "scissors", "scorpion", "scout", "scrap",
    "screen", "script", "scrub", "sea", "search", "season", "seat", "second", "secret",
    "section", "security", "seed", "seek", "segment", "select", "sell", "seminar", "senior",
    "sense", "sentence", "series", "service", "session", "settle", "setup", "seven", "shadow",
    "shaft", "shallow", "share", "shed", "shell", "sheriff", "shield", "shift", "shine",
    "ship", "shiver", "shock", "shoe", "shoot", "shop", "short", "shoulder", "shove",
    "shrimp", "shrug", "shuffle", "shy", "sibling", "sick", "side", "siege", "sight",
    "sign", "silent", "silk", "silly", "silver", "similar", "simple", "since", "sing",
    "siren", "sister", "situate", "six", "size", "skate", "sketch", "ski", "skill",
    "skin", "skirt", "skull", "slab", "slam", "sleep", "slender", "slice", "slide",
    "slight", "slim", "slogan", "slot", "slow", "slush", "small", "smart", "smile",
    "smoke", "smooth", "snack", "snake", "snap", "sniff", "snow", "soap", "soccer",
    "social", "sock", "soda", "soft", "solar", "soldier", "solid", "solution", "solve",
    "someone", "song", "soon", "sorry", "sort", "soul", "sound", "soup", "source",
    "south", "space", "spare", "spatial", "spawn", "speak", "special", "speed", "spell",
    "spend", "sphere", "spice", "spider", "spike", "spin", "spirit", "split", "spoil",
    "sponsor", "spoon", "sport", "spot", "spray", "spread", "spring", "spy", "square",
    "squeeze", "squirrel", "stable", "stadium", "staff", "stage", "stairs", "stamp", "stand",
    "start", "state", "stay", "steak", "steel", "stem", "step", "stereo", "stick",
    "still", "sting", "stock", "stomach", "stone", "stool", "story", "stove", "strategy",
    "street", "strike", "strong", "struggle", "student", "stuff", "stumble", "style", "subject",
    "submit", "subway", "success", "such", "sudden", "suffer", "sugar", "suggest", "suit",
    "summer", "sun", "sunny", "sunset", "super", "supply", "supreme", "sure", "surface",
    "surge", "surprise", "surround", "survey", "suspect", "sustain", "swallow", "swamp", "swap",
    "swarm", "swear", "sweet", "swift", "swim", "swing", "switch", "sword", "symbol",
    "symptom", "syrup", "system", "table", "tackle", "tag", "tail", "talent", "talk",
    "tank", "tape", "target", "task", "taste", "tattoo", "taxi", "teach", "team",
    "tell", "ten", "tenant", "tennis", "tent", "term", "test", "text", "thank",
    "that", "theme", "then", "theory", "there", "they", "thing", "this", "thought",
    "three", "thrive", "throw", "thumb", "thunder", "ticket", "tide", "tiger", "tilt",
    "timber", "time", "tiny", "tip", "tired", "tissue", "title", "toast", "tobacco",
    "today", "toddler", "toe", "together", "toilet", "token", "tomato", "tomorrow", "tone",
    "tongue", "tonight", "tool", "tooth", "top", "topic", "topple", "torch", "tornado",
    "tortoise", "toss", "total", "tourist", "toward", "tower", "town", "toy", "track",
    "trade", "traffic", "tragic", "train", "transfer", "trap", "trash", "travel", "tray",
    "treat", "tree", "trend", "trial", "tribe", "trick", "trigger", "trim", "trip",
    "trophy", "trouble", "truck", "true", "truly", "trumpet", "trust", "truth", "try",
    "tube", "tuition", "tumble", "tuna", "tunnel", "turkey", "turn", "turtle", "twelve",
    "twenty", "twice", "twin", "twist", "two", "type", "typical", "ugly", "umbrella",
    "unable", "unaware", "uncle", "uncover", "under", "undo", "unfair", "unfold", "unhappy",
    "uniform", "unique", "unit", "universe", "unknown", "unlock", "until", "unusual", "unveil",
    "update", "upgrade", "uphold", "upon", "upper", "upset", "urban", "urge", "usage",
    "use", "used", "useful", "useless", "usual", "utility", "vacant", "vacuum", "vague",
    "valid", "valley", "valve", "van", "vanish", "vapor", "various", "vast", "vault",
    "vehicle", "velvet", "vendor", "venture", "venue", "verb", "verify", "version", "very",
    "vessel", "veteran", "viable", "vibrant", "vicious", "victory", "video", "view", "village",
    "vintage", "violin", "virtual", "virus", "visa", "visit", "visual", "vital", "vivid",
    "vocal", "voice", "void", "volcano", "volume", "vote", "voyage", "wage", "wagon",
    "wait", "walk", "wall", "walnut", "want", "warfare", "warm", "warrior", "wash",
    "wasp", "waste", "water", "wave", "way", "wealth", "weapon", "wear", "weasel",
    "weather", "web", "wedding", "weekend", "weird", "welcome", "west", "wet", "whale",
    "what", "wheat", "wheel", "when", "where", "whip", "whisper", "wide", "width",
    "wife", "wild", "will", "win", "window", "wine", "wing", "wink", "winner",
    "winter", "wire", "wisdom", "wise", "wish", "witness", "wolf", "woman", "wonder",
    "wood", "wool", "word", "work", "world", "worry", "worth", "wrap", "wreck",
    "wrestle", "wrist", "write", "wrong", "yard", "year", "yellow", "you", "young",
    "youth", "zebra", "zero", "zone", "zoo",
];

/// Encode a backup shard as a BIP-39-style mnemonic phrase.
///
/// For a 32-byte shard, produces 24 words with checksum.
pub fn encode_as_mnemonic(shard: &DecryptedShard) -> Result<Vec<String>> {
    let bytes = shard.as_bytes();
    if bytes.len() != 32 {
        return Err(MpcError::ShardEncryption(format!(
            "shard must be 32 bytes for BIP-39 encoding, got {}",
            bytes.len()
        )));
    }

    // SHA-256 checksum of the entropy
    let hash = Sha256::digest(bytes);
    let checksum_bits = 8; // 32 bytes = 256 bits -> 8-bit checksum -> 264 total bits -> 24 words

    // Convert entropy + checksum into 11-bit word indices
    let mut bits = Vec::with_capacity(264);

    // Add entropy bits (256 bits)
    for &byte in bytes {
        for i in 0..8 {
            bits.push((byte >> (7 - i)) & 1);
        }
    }

    // Add checksum bits (8 bits)
    let checksum_byte = hash[0];
    for i in 0..checksum_bits {
        bits.push((checksum_byte >> (7 - i)) & 1);
    }

    // Convert 11-bit chunks to words
    let mut words = Vec::with_capacity(24);
    for chunk in bits.chunks(11) {
        let mut index = 0u16;
        for (i, &bit) in chunk.iter().enumerate() {
            index |= (bit as u16) << (10 - i);
        }
        words.push(BIP39_WORDS[index as usize].to_string());
    }

    Ok(words)
}

/// Decode a BIP-39 mnemonic phrase back into a backup shard.
pub fn decode_from_mnemonic(words: &[String]) -> Result<DecryptedShard> {
    if words.len() != 24 {
        return Err(MpcError::ShardDecryption(format!(
            "BIP-39 32-byte shard requires 24 words, got {}",
            words.len()
        )));
    }

    // Convert words to 11-bit indices
    let mut bits = Vec::with_capacity(264);

    for word in words {
        let index = BIP39_WORDS
            .iter()
            .position(|&w| w == word.as_str())
            .ok_or_else(|| MpcError::ShardDecryption(format!("invalid BIP-39 word: {}", word)))?;

        let index = index as u16;
        for i in 0..11 {
            bits.push(((index >> (10 - i)) & 1) as u8);
        }
    }

    // Extract first 256 bits as entropy
    let mut entropy = vec![0u8; 32];
    for (i, byte) in entropy.iter_mut().enumerate() {
        for j in 0..8 {
            *byte |= bits[i * 8 + j] << (7 - j);
        }
    }

    // Verify checksum
    let hash = Sha256::digest(&entropy);
    let checksum_byte = hash[0];

    for i in 0..8 {
        let expected = (checksum_byte >> (7 - i)) & 1;
        let actual = bits[256 + i];
        if expected != actual {
            return Err(MpcError::ShardDecryption(
                "invalid mnemonic checksum".into(),
            ));
        }
    }

    Ok(DecryptedShard::from_bytes(entropy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeroize::Zeroize;

    #[test]
    fn test_gf256_arithmetic() {
        // Test addition (XOR)
        assert_eq!(gf256::add(5, 3), 6);
        assert_eq!(gf256::add(0, 0), 0);
        assert_eq!(gf256::add(255, 255), 0);

        // Test multiplication
        assert_eq!(gf256::mul(0, 42), 0);
        assert_eq!(gf256::mul(1, 42), 42);

        // Test inverse
        for x in 1..=255u8 {
            let inv = gf256::inverse(x);
            assert_eq!(gf256::mul(x, inv), 1);
        }
    }

    #[test]
    fn test_shamir_split_reconstruct_basic() {
        let secret = vec![0x42u8; 32];
        let shard = DecryptedShard::from_bytes(secret.clone());

        let shares = split_for_social_recovery(&shard, 3, 5).unwrap();
        assert_eq!(shares.len(), 5);

        // Check all shares have correct metadata
        for share in &shares {
            assert_eq!(share.threshold, 3);
            assert_eq!(share.total, 5);
            assert_eq!(share.data.len(), 32);
        }

        // Reconstruct from any 3 shares
        let reconstructed = reconstruct_from_shares(&shares[0..3]).unwrap();
        assert_eq!(reconstructed.as_bytes(), secret.as_slice());

        // Another combination
        let reconstructed = reconstruct_from_shares(&shares[2..5]).unwrap();
        assert_eq!(reconstructed.as_bytes(), secret.as_slice());
    }

    #[test]
    fn test_shamir_insufficient_shares() {
        let secret = vec![0x42u8; 32];
        let shard = DecryptedShard::from_bytes(secret);

        let shares = split_for_social_recovery(&shard, 3, 5).unwrap();

        // Only 2 shares should fail
        let result = reconstruct_from_shares(&shares[0..2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_shamir_different_secrets() {
        for secret_val in 0..5u8 {
            let secret = vec![secret_val; 32];
            let shard = DecryptedShard::from_bytes(secret.clone());

            let shares = split_for_social_recovery(&shard, 2, 5).unwrap();
            let reconstructed = reconstruct_from_shares(&shares[1..3]).unwrap();

            assert_eq!(reconstructed.as_bytes(), secret.as_slice());
        }
    }

    #[test]
    fn test_mnemonic_roundtrip() {
        // Test multiple different secrets
        let test_cases = vec![
            vec![0x00u8; 32], // All zeros
            vec![0xFFu8; 32], // All ones
            (0..32).collect(), // Incrementing
            {
                let mut v = vec![0u8; 32];
                for i in 0..32 {
                    v[i] = (i * 7) as u8;
                }
                v
            },
        ];

        for secret in test_cases {
            let shard = DecryptedShard::from_bytes(secret.clone());
            let words = encode_as_mnemonic(&shard).unwrap();
            assert_eq!(words.len(), 24);

            let decoded = decode_from_mnemonic(&words).unwrap();
            assert_eq!(decoded.as_bytes(), secret.as_slice());

            // Zeroize
            let mut secret_mut = secret;
            secret_mut.zeroize();
        }
    }

    #[test]
    fn test_mnemonic_invalid_word() {
        let secret = vec![0x42u8; 32];
        let shard = DecryptedShard::from_bytes(secret);
        let mut words = encode_as_mnemonic(&shard).unwrap();

        // Corrupt a word
        words[0] = "invalidwordxyz".to_string();

        let result = decode_from_mnemonic(&words);
        assert!(result.is_err());
    }

    #[test]
    fn test_mnemonic_invalid_checksum() {
        let secret = vec![0x42u8; 32];
        let shard = DecryptedShard::from_bytes(secret);
        let mut words = encode_as_mnemonic(&shard).unwrap();

        // Swap two words (breaks checksum)
        words.swap(0, 1);

        let result = decode_from_mnemonic(&words);
        assert!(result.is_err());
    }

    #[test]
    fn test_mnemonic_wrong_length() {
        let result = decode_from_mnemonic(&["hello".to_string()]);
        assert!(result.is_err());

        let mut twelve_words = Vec::new();
        for i in 0..12 {
            twelve_words.push(BIP39_WORDS[i].to_string());
        }
        let result = decode_from_mnemonic(&twelve_words);
        assert!(result.is_err());
    }
}
