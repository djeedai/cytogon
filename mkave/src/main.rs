use cellular::*;

fn main() {
    println!("cave");

    //let mut cave = Cave2::new(UVec2::new(128, 32), 0.45);
    let mut cave = Cave3::new(UVec3::new(8, 8, 3), 0.45);
    println!("{}", export_txt3(cave.size, &cave.data));
    cave.smooth();
    // cave.smooth();
    // cave.smooth();
    // cave.smooth();
    // cave.smooth();
    //println!("{}", export_txt2(cave.size, &cave.data));
    println!("{}", export_txt3(cave.size, &cave.data));
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

fn export_txt3(size: UVec3, data: &[bool]) -> String {
    let mut s = String::with_capacity(data.len() + size.y as usize + size.z as usize);
    let mut i = 0;
    let mut j = 0;
    for b in data {
        s.push(if *b { '#' } else { ' ' });
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
    s
}
