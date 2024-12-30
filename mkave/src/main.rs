use cytogon::*;

fn main() {
    println!("cave");

    //let mut cave = Grid2::new(UVec2::new(128, 32), 0.45);
    //let mut cave = Grid3::new(UVec3::new(8, 8, 3));
    let mut cave = Grid3::new(UVec3::new(128, 128, 128));
    cave.fill_rand(0.6, rand::thread_rng());
    //println!("{}", export_txt3(cave.size, &cave.data));

    // 13-26/13-14,17-19/2/M
    let rule = Rule3 {
        birth: RuleBitset3::from(13u8..=14u8) | (17u8..=19u8).into(),
        survive: (13u8..=26u8).into(),
    };
    cave.smooth(&rule);
    cave.smooth(&rule);
    cave.smooth(&rule);
    cave.smooth(&rule);
    cave.smooth(&rule);
    //println!("{}", export_txt2(cave.size, &cave.data));
    //println!("{}", export_txt3(cave.size, &cave.data));
}

fn export_txt2(size: UVec2, data: &[bool]) -> String {
    let mut s = String::with_capacity(data.len() + size.y as usize);
    let mut i = 0;
    for b in data {
        s.push(if *b { '#' } else { ' ' });
        i += 1;
        if i == size.x {
            i = 0;
            s.push('\n');
        }
    }
    s
}

fn export_txt3(size: UVec3, data: &[u64]) -> String {
    let mut s = String::with_capacity(data.len() + size.y as usize + size.z as usize);
    let mut i = 0;
    let mut j = 0;
    for block in data {
        for bit in 0..63 {
            let b = block & (1u64 << bit) != 0;
            s.push(if b { '#' } else { ' ' });
            i += 1;
            if i == size.x {
                i = 0;
                s.push('\n');
                j += 1;
                if j == size.y {
                    j = 0;
                    s.push('\n');
                }
            }
        }
    }
    s
}
