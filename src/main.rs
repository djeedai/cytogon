use glam::{UVec2, UVec3};
use rand::Rng;

struct Cave2 {
    pub size: UVec2,
    pub data: Vec<bool>,
}

impl Cave2 {
    pub fn new(size: UVec2, fill_ratio: f32) -> Self {
        Self {
            size,
            data: fill(size.extend(1), fill_ratio),
        }
    }

    pub fn smooth(&mut self) {
        smooth2(self.size, &mut self.data);
    }
}

struct Cave3 {
    pub size: UVec3,
    pub data: Vec<bool>,
}

impl Cave3 {
    pub fn new(size: UVec3, fill_ratio: f32) -> Self {
        Self {
            size,
            data: fill(size, fill_ratio),
        }
    }

    pub fn smooth(&mut self) {
        smooth3(self.size, &mut self.data);
    }
}

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

fn fill(size: UVec3, fill_ratio: f32) -> Vec<bool> {
    let capacity = (size.x * size.y * size.z) as usize;
    let mut data = Vec::with_capacity(capacity);
    for _ in 0..capacity {
        let p: f32 = rand::thread_rng().gen_range(0.0..1.0);
        data.push(p < fill_ratio);
    }
    data
}

fn count_neighbor_walls2(size: UVec2, data: &[bool], pos: UVec2) -> u8 {
    let mut count = 0;
    if pos.x == 0 || data[(pos.y * size.x + pos.x - 1) as usize] {
        count += 1;
    }
    if pos.x == size.x - 1 || data[(pos.y * size.x + pos.x + 1) as usize] {
        count += 1;
    }
    if pos.y == 0 || data[((pos.y - 1) * size.x + pos.x) as usize] {
        count += 1;
    }
    if pos.y == size.y - 1 || data[((pos.y + 1) * size.x + pos.x) as usize] {
        count += 1;
    }
    if (pos.x == 0 && pos.y == 0) || data[((pos.y - 1) * size.x + pos.x - 1) as usize] {
        count += 1;
    }
    if (pos.x == size.x - 1 && pos.y == 0) || data[((pos.y - 1) * size.x + pos.x + 1) as usize] {
        count += 1;
    }
    if (pos.x == 0 && pos.y == size.y - 1) || data[((pos.y + 1) * size.x + pos.x - 1) as usize] {
        count += 1;
    }
    if (pos.x == size.x - 1 && pos.y == size.y - 1)
        || data[((pos.y + 1) * size.x + pos.x + 1) as usize]
    {
        count += 1;
    }
    count
}

fn smooth2(size: UVec2, data: &mut [bool]) {
    for j in 0..size.y {
        for i in 0..size.x {
            if i == 0 || j == 0 || i == size.x - 1 || j == size.y - 1 {
                data[(j * size.x + i) as usize] = true;
            } else {
                // 5-8/5-8/2/M (?)
                let c = count_neighbor_walls2(size, data, UVec2::new(i, j));
                if c > 4 {
                    data[(j * size.x + i) as usize] = true;
                } else if c < 4 {
                    data[(j * size.x + i) as usize] = false;
                }
            }
        }
    }
}

fn count_neighbor_walls3(size: UVec3, data: &[bool], pos: UVec3) -> u8 {
    let mut count = 0;
    let pos = pos.as_ivec3();
    for k in (pos.z - 1)..(pos.z + 1) {
        if k < 0 || k >= size.z as i32 {
            count += 1;
        } else {
            for j in (pos.y - 1)..(pos.y + 1) {
                if j < 0 || j >= size.y as i32 {
                    count += 1;
                } else {
                    for i in (pos.x - 1)..(pos.x + 1) {
                        if i == pos.x && j == pos.y && k == pos.z {
                            // skip self
                            continue;
                        }
                        if i < 0 || i >= size.x as i32 {
                            count += 1;
                        } else {
                            let index = k as u32 * size.y * size.x + j as u32 * size.x + i as u32;
                            if data[index as usize] {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    count
}

fn smooth3(size: UVec3, data: &mut [bool]) {
    for k in 0..size.z {
        for j in 0..size.y {
            for i in 0..size.x {
                if i == 0 || j == 0 || i == size.x - 1 || j == size.y - 1 {
                    data[(j * size.x + i) as usize] = true;
                } else {
                    // 13-26/13-14,17-19/2/M
                    let c = count_neighbor_walls3(size, data, UVec3::new(i, j, k));
                    let cell = &mut data[(k * size.y * size.x + j * size.x + i) as usize];
                    if *cell {
                        // Alive cell with 13+ neighbors survive
                        *cell = c >= 13;
                    } else {
                        // Empty cell with 13-14 or 17-19 neighbors have a new cell
                        *cell = (c == 13) || (c == 14) || (c >= 17 && c <= 19);
                    }
                }
            }
        }
    }
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
