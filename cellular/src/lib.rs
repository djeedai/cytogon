pub use glam::{IVec2, IVec3, UVec2, UVec3};
use rand::{Rng, RngCore};

pub struct Grid2 {
    pub size: UVec2,
    pub data: Vec<bool>,
}

impl Grid2 {
    pub fn new(size: UVec2) -> Self {
        Self { size, data: vec![] }
    }

    pub fn fill(&mut self, value: bool) {
        self.data
            .resize((self.size.x * self.size.y) as usize, value);
    }

    pub fn fill_rand(&mut self, fill_ratio: f32, mut prng: impl RngCore) {
        self.data = fill(self.size.extend(1), fill_ratio, &mut prng);
    }

    #[inline]
    pub fn cell(&self, pos: IVec2) -> Option<bool> {
        if pos.x < 0 || pos.y < 0 || pos.x as u32 >= self.size.x || pos.y as u32 >= self.size.y {
            None
        } else {
            let index = pos.y as u32 * self.size.x + pos.x as u32;
            Some(self.data[index as usize])
        }
    }

    #[inline]
    pub fn cell_mut(&mut self, pos: IVec2) -> Option<&mut bool> {
        if pos.x < 0 || pos.y < 0 || pos.x as u32 >= self.size.x || pos.y as u32 >= self.size.y {
            None
        } else {
            let index = pos.y as u32 * self.size.x + pos.x as u32;
            Some(&mut self.data[index as usize])
        }
    }

    pub fn smooth(&mut self) {
        let imax = self.size.x - 1;
        let jmax = self.size.y - 1;
        let default = false;
        for j in 0..=jmax {
            for i in 0..=imax {
                let pos = IVec2::new(i as i32, j as i32);
                if default && (i == 0 || j == 0 || i == imax || j == jmax) {
                    let cell = self.cell_mut(pos).unwrap();
                    *cell = true;
                } else {
                    // 5-8/5-8/2/M (?)
                    let c = self.count_neighbors(pos, default);
                    if c > 4 {
                        let cell = self.cell_mut(pos).unwrap();
                        *cell = true;
                    } else if c < 4 {
                        let cell = self.cell_mut(pos).unwrap();
                        *cell = false;
                    }
                }
            }
        }
    }

    fn count_neighbors(&self, pos: IVec2, default: bool) -> u8 {
        let mut count = 0;
        let mut xy = pos;
        for j in (pos.y - 1)..=(pos.y + 1) {
            xy.y = j;
            for i in (pos.x - 1)..=(pos.x + 1) {
                xy.x = i;
                if xy != pos && self.cell(xy).unwrap_or(default) {
                    count += 1;
                }
            }
        }
        count
    }
}

pub struct Grid3 {
    pub size: UVec3,
    pub data: Vec<bool>,
}

impl Grid3 {
    pub fn new(size: UVec3) -> Self {
        Self { size, data: vec![] }
    }

    pub fn fill(&mut self, value: bool) {
        self.data
            .resize((self.size.x * self.size.y * self.size.z) as usize, value);
    }

    pub fn fill_rand(&mut self, fill_ratio: f32, mut prng: impl RngCore) {
        self.data = fill(self.size, fill_ratio, &mut prng);
    }

    #[inline]
    pub fn cell(&self, pos: IVec3) -> Option<bool> {
        if pos.x < 0
            || pos.y < 0
            || pos.z < 0
            || pos.x as u32 >= self.size.x
            || pos.y as u32 >= self.size.y
            || pos.z as u32 >= self.size.z
        {
            None
        } else {
            let index = pos.z as u32 * self.size.y * self.size.x
                + pos.y as u32 * self.size.x
                + pos.x as u32;
            Some(self.data[index as usize])
        }
    }

    #[inline]
    pub fn cell_mut(&mut self, pos: IVec3) -> Option<&mut bool> {
        if pos.x < 0
            || pos.y < 0
            || pos.z < 0
            || pos.x as u32 >= self.size.x
            || pos.y as u32 >= self.size.y
            || pos.z as u32 >= self.size.z
        {
            None
        } else {
            let index = pos.z as u32 * self.size.y * self.size.x
                + pos.y as u32 * self.size.x
                + pos.x as u32;
            Some(&mut self.data[index as usize])
        }
    }

    pub fn smooth(&mut self) {
        let imax = self.size.x - 1;
        let jmax = self.size.y - 1;
        let kmax = self.size.z - 1;
        let default = false;
        for k in 0..=kmax {
            for j in 0..=jmax {
                for i in 0..=imax {
                    let pos = IVec3::new(i as i32, j as i32, k as i32);
                    if default
                        && (i == 0 || j == 0 || k == 0 || i == imax || j == jmax || k == kmax)
                    {
                        let cell = self.cell_mut(pos).unwrap();
                        *cell = true;
                    } else {
                        // 13-26/13-14,17-19/2/M
                        let c = self.count_neighbors(pos, default);
                        let cell = self.cell_mut(pos).unwrap();
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

    fn count_neighbors(&self, pos: IVec3, default: bool) -> u8 {
        let mut count = 0;
        let mut xyz = pos;
        for k in (pos.z - 1)..=(pos.z + 1) {
            xyz.z = k;
            for j in (pos.y - 1)..=(pos.y + 1) {
                xyz.y = j;
                for i in (pos.x - 1)..=(pos.x + 1) {
                    xyz.x = i;
                    if xyz != pos && self.cell(xyz).unwrap_or(default) {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

fn fill(size: UVec3, fill_ratio: f32, mut prng: impl RngCore) -> Vec<bool> {
    let capacity = (size.x * size.y * size.z) as usize;
    let mut data = Vec::with_capacity(capacity);
    for _ in 0..capacity {
        let p: f32 = prng.gen_range(0.0..1.0);
        data.push(p < fill_ratio);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors3() {
        // 3x3x3 grid with alive cell in center
        let mut grid = Grid3::new(UVec3::ONE * 3);
        grid.fill(false);
        *grid.cell_mut(IVec3::ONE).unwrap() = true;

        for k in -1..=1 {
            for j in -1..=1 {
                for i in -1..=1 {
                    let pos = IVec3::new(i + 1, j + 1, k + 1);
                    if pos == IVec3::ONE {
                        // center: no neighbor
                        assert_eq!(grid.count_neighbors(pos, false), 0);
                    } else if i * j * k != 0 {
                        // corner: neighbor is center, and optionally out-of-bound values if default=true
                        assert_eq!(grid.count_neighbors(pos, false), 1);
                        assert_eq!(grid.count_neighbors(pos, true), 20);
                    } else if i * j != 0 || i * k != 0 || j * k != 0 {
                        // edge center: neighbor is center, and optionally out-of-bound values if default=true
                        assert_eq!(grid.count_neighbors(pos, false), 1);
                        assert_eq!(grid.count_neighbors(pos, true), 16);
                    } else {
                        // face center: neighbor is center, and optionally out-of-bound values if default=true
                        assert_eq!(grid.count_neighbors(pos, false), 1);
                        assert_eq!(grid.count_neighbors(pos, true), 10);
                    }
                }
            }
        }
    }
}
