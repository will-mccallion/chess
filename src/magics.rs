// src/magics.rs

use crate::types::Bitboard;
use std::path::Path;
use std::sync::OnceLock;

fn run_generator() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static IS_GENERATING: AtomicBool = AtomicBool::new(false);

    if IS_GENERATING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        println!("Magic bitboard attack files not found in 'moves/' directory.");
        println!("Generating them now. This is a one-time process and may take a few minutes...");

        crate::magic_finder::generate_magics_code();

        println!("Generation complete. The application will now continue.");
        IS_GENERATING.store(false, Ordering::SeqCst);
    } else {
        while IS_GENERATING.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
}

fn load_table(path_str: &str) -> &'static [Bitboard] {
    let path = Path::new(path_str);
    if !path.exists() {
        run_generator();
    }

    let bytes = std::fs::read(path)
        .unwrap_or_else(|e| panic!("Failed to read magic file {}: {}", path_str, e));

    let leaked_bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());

    unsafe {
        let ptr = leaked_bytes.as_ptr() as *const Bitboard;
        let len = leaked_bytes.len() / std::mem::size_of::<Bitboard>();
        std::slice::from_raw_parts(ptr, len)
    }
}

static ROOK_ATTACKS: OnceLock<&'static [Bitboard]> = OnceLock::new();
static BISHOP_ATTACKS: OnceLock<&'static [Bitboard]> = OnceLock::new();

fn get_rook_table() -> &'static [Bitboard] {
    ROOK_ATTACKS.get_or_init(|| load_table("moves/rook_attacks.bin"))
}

fn get_bishop_table() -> &'static [Bitboard] {
    BISHOP_ATTACKS.get_or_init(|| load_table("moves/bishop_attacks.bin"))
}

struct Magic {
    mask: Bitboard,
    magic: u64,
    shift: u32,
    offset: usize,
}

const ROOK_MAGICS: [Magic; 64] = [
    Magic {
        mask: 0x101010101017e,
        magic: 0x180008028d34000,
        shift: 52,
        offset: 0,
    },
    Magic {
        mask: 0x202020202027c,
        magic: 0x2040002000100044,
        shift: 53,
        offset: 4096,
    },
    Magic {
        mask: 0x404040404047a,
        magic: 0x200208200400810,
        shift: 53,
        offset: 6144,
    },
    Magic {
        mask: 0x8080808080876,
        magic: 0x7100080500201001,
        shift: 53,
        offset: 8192,
    },
    Magic {
        mask: 0x1010101010106e,
        magic: 0xa00200810020004,
        shift: 53,
        offset: 10240,
    },
    Magic {
        mask: 0x2020202020205e,
        magic: 0x8100080204000100,
        shift: 53,
        offset: 12288,
    },
    Magic {
        mask: 0x4040404040403e,
        magic: 0x3580008022000100,
        shift: 53,
        offset: 14336,
    },
    Magic {
        mask: 0x8080808080807e,
        magic: 0x8006450004a080,
        shift: 52,
        offset: 16384,
    },
    Magic {
        mask: 0x1010101017e00,
        magic: 0xc0800020400090,
        shift: 53,
        offset: 20480,
    },
    Magic {
        mask: 0x2020202027c00,
        magic: 0x9000401000402002,
        shift: 54,
        offset: 22528,
    },
    Magic {
        mask: 0x4040404047a00,
        magic: 0x160801000200080,
        shift: 54,
        offset: 23552,
    },
    Magic {
        mask: 0x8080808087600,
        magic: 0x3001001002008,
        shift: 54,
        offset: 24576,
    },
    Magic {
        mask: 0x10101010106e00,
        magic: 0x800400080080,
        shift: 54,
        offset: 25600,
    },
    Magic {
        mask: 0x20202020205e00,
        magic: 0x800400800200,
        shift: 54,
        offset: 26624,
    },
    Magic {
        mask: 0x40404040403e00,
        magic: 0x2141010002000401,
        shift: 54,
        offset: 27648,
    },
    Magic {
        mask: 0x80808080807e00,
        magic: 0x120500044600a900,
        shift: 53,
        offset: 28672,
    },
    Magic {
        mask: 0x10101017e0100,
        magic: 0x208000804000,
        shift: 53,
        offset: 30720,
    },
    Magic {
        mask: 0x20202027c0200,
        magic: 0x808020004013,
        shift: 54,
        offset: 32768,
    },
    Magic {
        mask: 0x40404047a0400,
        magic: 0xa0008020100080,
        shift: 54,
        offset: 33792,
    },
    Magic {
        mask: 0x8080808760800,
        magic: 0xb0004008004400,
        shift: 54,
        offset: 34816,
    },
    Magic {
        mask: 0x101010106e1000,
        magic: 0x4006020008211084,
        shift: 54,
        offset: 35840,
    },
    Magic {
        mask: 0x202020205e2000,
        magic: 0x3000808004000200,
        shift: 54,
        offset: 36864,
    },
    Magic {
        mask: 0x404040403e4000,
        magic: 0x40a0040090084102,
        shift: 54,
        offset: 37888,
    },
    Magic {
        mask: 0x808080807e8000,
        magic: 0x420020014806504,
        shift: 53,
        offset: 38912,
    },
    Magic {
        mask: 0x101017e010100,
        magic: 0x8000400580017080,
        shift: 53,
        offset: 40960,
    },
    Magic {
        mask: 0x202027c020200,
        magic: 0x4040200080400080,
        shift: 54,
        offset: 43008,
    },
    Magic {
        mask: 0x404047a040400,
        magic: 0x1400100480200480,
        shift: 54,
        offset: 44032,
    },
    Magic {
        mask: 0x8080876080800,
        magic: 0x5220200108940,
        shift: 54,
        offset: 45056,
    },
    Magic {
        mask: 0x1010106e101000,
        magic: 0x23000500080010,
        shift: 54,
        offset: 46080,
    },
    Magic {
        mask: 0x2020205e202000,
        magic: 0x800a008080020400,
        shift: 54,
        offset: 47104,
    },
    Magic {
        mask: 0x4040403e404000,
        magic: 0x401002100020084,
        shift: 54,
        offset: 48128,
    },
    Magic {
        mask: 0x8080807e808000,
        magic: 0x1004210200144484,
        shift: 53,
        offset: 49152,
    },
    Magic {
        mask: 0x1017e01010100,
        magic: 0x800400028800082,
        shift: 53,
        offset: 51200,
    },
    Magic {
        mask: 0x2027c02020200,
        magic: 0x21a200641401001,
        shift: 54,
        offset: 53248,
    },
    Magic {
        mask: 0x4047a04040400,
        magic: 0x84280a003801000,
        shift: 54,
        offset: 54272,
    },
    Magic {
        mask: 0x8087608080800,
        magic: 0x200100101002008,
        shift: 54,
        offset: 55296,
    },
    Magic {
        mask: 0x10106e10101000,
        magic: 0x9001000801001004,
        shift: 54,
        offset: 56320,
    },
    Magic {
        mask: 0x20205e20202000,
        magic: 0x1001002803000400,
        shift: 54,
        offset: 57344,
    },
    Magic {
        mask: 0x40403e40404000,
        magic: 0x104100104000208,
        shift: 54,
        offset: 58368,
    },
    Magic {
        mask: 0x80807e80808000,
        magic: 0x8000008842001914,
        shift: 53,
        offset: 59392,
    },
    Magic {
        mask: 0x17e0101010100,
        magic: 0x8000800040008020,
        shift: 53,
        offset: 61440,
    },
    Magic {
        mask: 0x27c0202020200,
        magic: 0x1a30144020004008,
        shift: 54,
        offset: 63488,
    },
    Magic {
        mask: 0x47a0404040400,
        magic: 0x10002804002000,
        shift: 54,
        offset: 64512,
    },
    Magic {
        mask: 0x8760808080800,
        magic: 0x1008001000808008,
        shift: 54,
        offset: 65536,
    },
    Magic {
        mask: 0x106e1010101000,
        magic: 0x2029001008010004,
        shift: 54,
        offset: 66560,
    },
    Magic {
        mask: 0x205e2020202000,
        magic: 0x2000409020010,
        shift: 54,
        offset: 67584,
    },
    Magic {
        mask: 0x403e4040404000,
        magic: 0x90420108040010,
        shift: 54,
        offset: 68608,
    },
    Magic {
        mask: 0x807e8080808000,
        magic: 0x58810080420004,
        shift: 53,
        offset: 69632,
    },
    Magic {
        mask: 0x7e010101010100,
        magic: 0x420402102008200,
        shift: 53,
        offset: 71680,
    },
    Magic {
        mask: 0x7c020202020200,
        magic: 0x10004000200840,
        shift: 54,
        offset: 73728,
    },
    Magic {
        mask: 0x7a040404040400,
        magic: 0x8011408020120600,
        shift: 54,
        offset: 74752,
    },
    Magic {
        mask: 0x76080808080800,
        magic: 0x7800480010008280,
        shift: 54,
        offset: 75776,
    },
    Magic {
        mask: 0x6e101010101000,
        magic: 0x2248000c004200c0,
        shift: 54,
        offset: 76800,
    },
    Magic {
        mask: 0x5e202020202000,
        magic: 0x2000811040200,
        shift: 54,
        offset: 77824,
    },
    Magic {
        mask: 0x3e404040404000,
        magic: 0x8100a990400,
        shift: 54,
        offset: 78848,
    },
    Magic {
        mask: 0x7e808080808000,
        magic: 0x1000240900964200,
        shift: 53,
        offset: 79872,
    },
    Magic {
        mask: 0x7e01010101010100,
        magic: 0x8880470010208001,
        shift: 52,
        offset: 81920,
    },
    Magic {
        mask: 0x7c02020202020200,
        magic: 0x1001200802042,
        shift: 53,
        offset: 86016,
    },
    Magic {
        mask: 0x7a04040404040400,
        magic: 0x4004200010084101,
        shift: 53,
        offset: 88064,
    },
    Magic {
        mask: 0x7608080808080800,
        magic: 0x400200409001001,
        shift: 53,
        offset: 90112,
    },
    Magic {
        mask: 0x6e10101010101000,
        magic: 0x622010490200802,
        shift: 53,
        offset: 92160,
    },
    Magic {
        mask: 0x5e20202020202000,
        magic: 0x140100084204006d,
        shift: 53,
        offset: 94208,
    },
    Magic {
        mask: 0x3e40404040404000,
        magic: 0x28080090010204,
        shift: 53,
        offset: 96256,
    },
    Magic {
        mask: 0x7e80808080808000,
        magic: 0x1022044100803402,
        shift: 52,
        offset: 98304,
    },
];

const BISHOP_MAGICS: [Magic; 64] = [
    Magic {
        mask: 0x40201008040200,
        magic: 0x401020280244c448,
        shift: 58,
        offset: 0,
    },
    Magic {
        mask: 0x402010080400,
        magic: 0x4024801010e0010,
        shift: 59,
        offset: 64,
    },
    Magic {
        mask: 0x4020100a00,
        magic: 0x21012400804100,
        shift: 59,
        offset: 96,
    },
    Magic {
        mask: 0x40221400,
        magic: 0x281a01a0280280,
        shift: 59,
        offset: 128,
    },
    Magic {
        mask: 0x2442800,
        magic: 0x184042020000100,
        shift: 59,
        offset: 160,
    },
    Magic {
        mask: 0x204085000,
        magic: 0x4064120202a4400,
        shift: 59,
        offset: 192,
    },
    Magic {
        mask: 0x20408102000,
        magic: 0x20840402400002,
        shift: 59,
        offset: 224,
    },
    Magic {
        mask: 0x2040810204000,
        magic: 0x6104802802100404,
        shift: 58,
        offset: 256,
    },
    Magic {
        mask: 0x20100804020000,
        magic: 0x2040080200c200,
        shift: 59,
        offset: 320,
    },
    Magic {
        mask: 0x40201008040000,
        magic: 0x20106401006201,
        shift: 59,
        offset: 352,
    },
    Magic {
        mask: 0x4020100a0000,
        magic: 0x4490604010000,
        shift: 59,
        offset: 384,
    },
    Magic {
        mask: 0x4022140000,
        magic: 0xe01482084200100,
        shift: 59,
        offset: 416,
    },
    Magic {
        mask: 0x244280000,
        magic: 0x4140060a10020080,
        shift: 59,
        offset: 448,
    },
    Magic {
        mask: 0x20408500000,
        magic: 0x482804100000,
        shift: 59,
        offset: 480,
    },
    Magic {
        mask: 0x2040810200000,
        magic: 0x4000200900411c2,
        shift: 59,
        offset: 512,
    },
    Magic {
        mask: 0x4081020400000,
        magic: 0x8829310888010800,
        shift: 59,
        offset: 544,
    },
    Magic {
        mask: 0x10080402000200,
        magic: 0x22001020025080,
        shift: 59,
        offset: 576,
    },
    Magic {
        mask: 0x20100804000400,
        magic: 0x81004040400d400,
        shift: 59,
        offset: 608,
    },
    Magic {
        mask: 0x4020100a000a00,
        magic: 0x401080e240010,
        shift: 57,
        offset: 640,
    },
    Magic {
        mask: 0x402214001400,
        magic: 0x3004000806a8a411,
        shift: 57,
        offset: 768,
    },
    Magic {
        mask: 0x24428002800,
        magic: 0x4050201210180,
        shift: 57,
        offset: 896,
    },
    Magic {
        mask: 0x2040850005000,
        magic: 0x202000090442024,
        shift: 57,
        offset: 1024,
    },
    Magic {
        mask: 0x4081020002000,
        magic: 0x180a108080b02808,
        shift: 59,
        offset: 1152,
    },
    Magic {
        mask: 0x8102040004000,
        magic: 0x30000240202a1,
        shift: 59,
        offset: 1184,
    },
    Magic {
        mask: 0x8040200020400,
        magic: 0x1130070108081009,
        shift: 59,
        offset: 1216,
    },
    Magic {
        mask: 0x10080400040800,
        magic: 0x2500082440809,
        shift: 59,
        offset: 1248,
    },
    Magic {
        mask: 0x20100a000a1000,
        magic: 0x402a010042240400,
        shift: 57,
        offset: 1280,
    },
    Magic {
        mask: 0x40221400142200,
        magic: 0x8884004008081100,
        shift: 55,
        offset: 1408,
    },
    Magic {
        mask: 0x2442800284400,
        magic: 0xd100840204802000,
        shift: 55,
        offset: 1920,
    },
    Magic {
        mask: 0x4085000500800,
        magic: 0x8004002005e00,
        shift: 57,
        offset: 2432,
    },
    Magic {
        mask: 0x8102000201000,
        magic: 0x2040810002011000,
        shift: 59,
        offset: 2560,
    },
    Magic {
        mask: 0x10204000402000,
        magic: 0x8004030008212702,
        shift: 59,
        offset: 2592,
    },
    Magic {
        mask: 0x4020002040800,
        magic: 0x8008603002182a00,
        shift: 59,
        offset: 2624,
    },
    Magic {
        mask: 0x8040004081000,
        magic: 0x12101000030200,
        shift: 59,
        offset: 2656,
    },
    Magic {
        mask: 0x100a000a102000,
        magic: 0x8704403000020400,
        shift: 57,
        offset: 2688,
    },
    Magic {
        mask: 0x22140014224000,
        magic: 0x20400820120200,
        shift: 55,
        offset: 2816,
    },
    Magic {
        mask: 0x44280028440200,
        magic: 0x8024010400020082,
        shift: 55,
        offset: 3328,
    },
    Magic {
        mask: 0x8500050080400,
        magic: 0x120004082410091,
        shift: 57,
        offset: 3840,
    },
    Magic {
        mask: 0x10200020100800,
        magic: 0x10008220008200,
        shift: 59,
        offset: 3968,
    },
    Magic {
        mask: 0x20400040201000,
        magic: 0x829002a084208,
        shift: 59,
        offset: 4000,
    },
    Magic {
        mask: 0x2000204081000,
        magic: 0x18080884408800,
        shift: 59,
        offset: 4032,
    },
    Magic {
        mask: 0x4000408102000,
        magic: 0x184008888400420,
        shift: 59,
        offset: 4064,
    },
    Magic {
        mask: 0xa000a10204000,
        magic: 0x4010c03c8000400,
        shift: 57,
        offset: 4096,
    },
    Magic {
        mask: 0x14001422400000,
        magic: 0x4440414208000082,
        shift: 57,
        offset: 4224,
    },
    Magic {
        mask: 0x28002844020000,
        magic: 0x200040010a00a101,
        shift: 57,
        offset: 4352,
    },
    Magic {
        mask: 0x50005008040200,
        magic: 0x8100081a00204,
        shift: 57,
        offset: 4480,
    },
    Magic {
        mask: 0x20002010080400,
        magic: 0x8052161222004c18,
        shift: 59,
        offset: 4608,
    },
    Magic {
        mask: 0x40004020100800,
        magic: 0x2044048800a00,
        shift: 59,
        offset: 4640,
    },
    Magic {
        mask: 0x20408102000,
        magic: 0x400840521090c,
        shift: 59,
        offset: 4672,
    },
    Magic {
        mask: 0x40810204000,
        magic: 0x600c60205200002,
        shift: 59,
        offset: 4704,
    },
    Magic {
        mask: 0xa1020400000,
        magic: 0x105100809000a0,
        shift: 59,
        offset: 4736,
    },
    Magic {
        mask: 0x142240000000,
        magic: 0xb0004084110000,
        shift: 59,
        offset: 4768,
    },
    Magic {
        mask: 0x284402000000,
        magic: 0x2000001002021800,
        shift: 59,
        offset: 4800,
    },
    Magic {
        mask: 0x500804020000,
        magic: 0x102040408021004,
        shift: 59,
        offset: 4832,
    },
    Magic {
        mask: 0x201008040200,
        magic: 0x2788a05414005800,
        shift: 59,
        offset: 4864,
    },
    Magic {
        mask: 0x402010080400,
        magic: 0x20380881004055,
        shift: 59,
        offset: 4896,
    },
    Magic {
        mask: 0x2040810204000,
        magic: 0xb406020080880908,
        shift: 58,
        offset: 4928,
    },
    Magic {
        mask: 0x4081020400000,
        magic: 0x1090108020208,
        shift: 59,
        offset: 4992,
    },
    Magic {
        mask: 0xa102040000000,
        magic: 0x508a103a0b000,
        shift: 59,
        offset: 5024,
    },
    Magic {
        mask: 0x14224000000000,
        magic: 0x103e00041208840,
        shift: 59,
        offset: 5056,
    },
    Magic {
        mask: 0x28440200000000,
        magic: 0x802002146208200,
        shift: 59,
        offset: 5088,
    },
    Magic {
        mask: 0x50080402000000,
        magic: 0x4890481072900101,
        shift: 59,
        offset: 5120,
    },
    Magic {
        mask: 0x20100804020000,
        magic: 0x23104410244044,
        shift: 59,
        offset: 5152,
    },
    Magic {
        mask: 0x40201008040200,
        magic: 0x220049420434200,
        shift: 58,
        offset: 5184,
    },
];

#[inline(always)]
pub fn get_rook_attacks(sq: usize, occupied: Bitboard) -> Bitboard {
    let magic = &ROOK_MAGICS[sq];
    let blockers = occupied & magic.mask;
    let index = (blockers.wrapping_mul(magic.magic) >> magic.shift) as usize;
    get_rook_table()[magic.offset + index]
}

#[inline(always)]
pub fn get_bishop_attacks(sq: usize, occupied: Bitboard) -> Bitboard {
    let magic = &BISHOP_MAGICS[sq];
    let blockers = occupied & magic.mask;
    let index = (blockers.wrapping_mul(magic.magic) >> magic.shift) as usize;
    get_bishop_table()[magic.offset + index]
}

const KNIGHT_ATTACKS: [Bitboard; 64] = [
    0x20400,
    0x50800,
    0xa1100,
    0x142200,
    0x284400,
    0x508800,
    0xa011000,
    0x4022000,
    0x2040004,
    0x5080008,
    0xa110011,
    0x14220022,
    0x28440044,
    0x50880088,
    0xa0110010,
    0x40220020,
    0x204000402,
    0x508000805,
    0xa1100110a,
    0x1422002214,
    0x2844004428,
    0x5088008850,
    0xa0110010a0,
    0x4022002040,
    0x20400040200,
    0x50800080500,
    0xa1100110a00,
    0x142200221400,
    0x284400442800,
    0x508800885000,
    0xa0110010a000,
    0x402200204000,
    0x2040004020000,
    0x5080008050000,
    0xa1100110a0000,
    0x14220022140000,
    0x28440044280000,
    0x50880088500000,
    0xa0110010a00000,
    0x40220020400000,
    0x4000402000000,
    0x8000805000000,
    0x1100110a000000,
    0x22002214000000,
    0x44004428000000,
    0x88008850000000,
    0x110010a0000000,
    0x22002040000000,
    0x40200000000,
    0x80500000000,
    0x110a00000000,
    0x221400000000,
    0x442800000000,
    0x885000000000,
    0x10a000000000,
    0x204000000000,
    0x400000000,
    0x800000000,
    0x1100000000,
    0x2200000000,
    0x4400000000,
    0x8800000000,
    0x1000000000,
    0x2000000000,
];

const KING_ATTACKS: [Bitboard; 64] = [
    0x302,
    0x705,
    0xe0a,
    0x1c14,
    0x3828,
    0x7050,
    0xe0a0,
    0xc040,
    0x30203,
    0x70507,
    0xe0a0e,
    0x1c141c,
    0x382838,
    0x705070,
    0xe0a0e0,
    0xc040c0,
    0x3020300,
    0x7050700,
    0xe0a0e00,
    0x1c141c00,
    0x38283800,
    0x70507000,
    0xe0a0e000,
    0xc040c000,
    0x302030000,
    0x705070000,
    0xe0a0e0000,
    0x1c141c0000,
    0x3828380000,
    0x7050700000,
    0xe0a0e00000,
    0xc040c00000,
    0x30203000000,
    0x70507000000,
    0xe0a0e000000,
    0x1c141c000000,
    0x382838000000,
    0x705070000000,
    0xe0a0e0000000,
    0xc040c0000000,
    0x3020300000000,
    0x7050700000000,
    0xe0a0e00000000,
    0x1c141c00000000,
    0x38283800000000,
    0x70507000000000,
    0xe0a0e000000000,
    0xc040c000000000,
    0x302030000000000,
    0x705070000000000,
    0xe0a0e0000000000,
    0x1c141c0000000000,
    0x3828380000000000,
    0x7050700000000000,
    0xe0a0e00000000000,
    0xc040c00000000000,
    0x203000000000000,
    0x507000000000000,
    0xa0e000000000000,
    0x141c000000000000,
    0x2838000000000000,
    0x5070000000000000,
    0xa0e0000000000000,
    0x40c0000000000000,
];

// NEW: Added pawn attack tables
pub const WHITE_PAWN_ATTACKS: [Bitboard; 64] = [
    0x2,
    0x5,
    0xa,
    0x14,
    0x28,
    0x50,
    0xa0,
    0x40,
    0x200,
    0x500,
    0xa00,
    0x1400,
    0x2800,
    0x5000,
    0xa000,
    0x4000,
    0x20000,
    0x50000,
    0xa0000,
    0x140000,
    0x280000,
    0x500000,
    0xa00000,
    0x400000,
    0x2000000,
    0x5000000,
    0xa000000,
    0x14000000,
    0x28000000,
    0x50000000,
    0xa0000000,
    0x40000000,
    0x200000000,
    0x500000000,
    0xa00000000,
    0x1400000000,
    0x2800000000,
    0x5000000000,
    0xa000000000,
    0x4000000000,
    0x20000000000,
    0x50000000000,
    0xa0000000000,
    0x140000000000,
    0x280000000000,
    0x500000000000,
    0xa00000000000,
    0x400000000000,
    0x2000000000000,
    0x5000000000000,
    0xa000000000000,
    0x14000000000000,
    0x28000000000000,
    0x50000000000000,
    0xa0000000000000,
    0x40000000000000,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
];

pub const BLACK_PAWN_ATTACKS: [Bitboard; 64] = [
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0x2,
    0x5,
    0xa,
    0x14,
    0x28,
    0x50,
    0xa0,
    0x40,
    0x200,
    0x500,
    0xa00,
    0x1400,
    0x2800,
    0x5000,
    0xa000,
    0x4000,
    0x20000,
    0x50000,
    0xa0000,
    0x140000,
    0x280000,
    0x500000,
    0xa00000,
    0x400000,
    0x2000000,
    0x5000000,
    0xa000000,
    0x14000000,
    0x28000000,
    0x50000000,
    0xa0000000,
    0x40000000,
    0x200000000,
    0x500000000,
    0xa00000000,
    0x1400000000,
    0x2800000000,
    0x5000000000,
    0xa000000000,
    0x4000000000,
    0x20000000000,
    0x50000000000,
    0xa0000000000,
    0x140000000000,
    0x280000000000,
    0x500000000000,
    0xa00000000000,
    0x400000000000,
    0x2000000000000,
    0x5000000000000,
    0xa000000000000,
    0x14000000000000,
    0x28000000000000,
    0x50000000000000,
    0xa0000000000000,
    0x40000000000000,
];

#[inline(always)]
pub fn knight_attacks_from(sq: usize) -> Bitboard {
    KNIGHT_ATTACKS[sq]
}

#[inline(always)]
pub fn king_attacks_from(sq: usize) -> Bitboard {
    KING_ATTACKS[sq]
}
