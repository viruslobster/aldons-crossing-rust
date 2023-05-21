//! The "fog of war" that obsures unexplored areas
use crate::{game::InvalidDataError, thrift::save};
use std::collections::{BTreeMap, HashMap};

pub(crate) struct Fog {
    last_look: Option<(u16, u16)>,
    current_map: u16,
    fog_by_map: HashMap<u16, [bool; 576]>,
}

impl Fog {
    pub fn new() -> Self {
        Self {
            last_look: None,
            fog_by_map: HashMap::new(),
            current_map: 0,
        }
    }

    pub fn save(&self) -> save::Fog {
        //Some(self.fog_by_map.clone()))
        let fog_by_map: BTreeMap<i32, Vec<bool>> = self
            .fog_by_map
            .iter()
            .map(|(key, value)| (*key as i32, value.to_vec()))
            .collect();

        save::Fog::new(self.current_map as i32, fog_by_map)
    }

    pub fn from_save(save: &save::Fog) -> Result<Self, InvalidDataError> {
        let fog_by_map: HashMap<u16, [bool; 576]> = save
            .fog_by_map
            .as_ref()
            .ok_or(InvalidDataError::new("fog_by_map field missing"))?
            .iter()
            .map(|(key, value)| (*key as u16, to_array(value)))
            .collect();

        let current_map = save
            .current_map
            .ok_or(InvalidDataError::new("current_map field missing"))?;

        let result = Self {
            last_look: None,
            fog_by_map,
            current_map: current_map as u16,
        };
        Ok(result)
    }

    fn idx(x: u16, y: u16) -> usize {
        (y * 24 + x) as usize
    }

    /// This looks at an eighth of the tiles around the player. This function is applied 8 times
    /// with different transforms to look at all tiles around the player.
    fn look_eighth<F>(&mut self, transform: F, sight_blocker: &[bool; 576])
    where
        F: Fn(u16, u16) -> (u16, u16) + Clone,
    {
        let mut vision = Vision {
            sight_blocker,
            transform: transform.clone(),
            cache: [None; 15],
        };
        let fog = self.fog_by_map.get_mut(&self.current_map).expect(&format!(
            "to have fog for the current map ({})",
            self.current_map
        ));

        for x in 0..5 {
            for y in 0..(x + 1) {
                let (xt, yt) = transform(x, y);
                if xt >= 24 || yt >= 24 {
                    continue;
                }
                let i = Self::idx(xt, yt);
                fog[i] &= !vision.visible(x, y);
            }
        }
    }

    /// Updates vision with the player looking from (x, y)
    pub fn look(&mut self, x: f64, y: f64, vision: &[bool; 576]) {
        let x = x.floor() as u16;
        let y = y.floor() as u16;
        if let Some((last_x, last_y)) = self.last_look {
            if (last_x, last_y) == (x, y) {
                return;
            }
        }
        self.last_look = Some((x, y));
        self.look_eighth(|xp, yp| (x + xp, y + yp), vision);
        self.look_eighth(|xp, yp| (x + yp, y + xp), vision);
        self.look_eighth(|xp, yp| (x + xp, y - yp), vision);
        self.look_eighth(|xp, yp| (x + yp, y - xp), vision);
        self.look_eighth(|xp, yp| (x - yp, y - xp), vision);
        self.look_eighth(|xp, yp| (x - xp, y - yp), vision);
        self.look_eighth(|xp, yp| (x - xp, y + yp), vision);
        self.look_eighth(|xp, yp| (x - yp, y + xp), vision);
    }

    pub fn occluded(&self, x: f64, y: f64) -> bool {
        let fog = self.fog_by_map.get(&self.current_map).expect(&format!(
            "to have fog for the current map ({})",
            self.current_map
        ));

        let i = Fog::idx(x as u16, y as u16);
        return fog[i];
    }

    pub fn load_map(&mut self, map_id: u16) {
        if !self.fog_by_map.contains_key(&map_id) {
            self.fog_by_map.insert(map_id, [true; 576]);
        }
        self.current_map = map_id;
        self.last_look = None;
    }

    pub fn current(&self) -> [bool; 576] {
        let fog = self.fog_by_map.get(&self.current_map).expect(&format!(
            "to have fog for the current map ({})",
            self.current_map
        ));
        *fog
    }
}

/// Implementation for computing what squares are visible
struct Vision<'a, F>
where
    F: Fn(u16, u16) -> (u16, u16) + Clone,
{
    sight_blocker: &'a [bool; 576],
    transform: F,
    cache: [Option<bool>; 15],
}

impl<'a, F> Vision<'a, F>
where
    F: Fn(u16, u16) -> (u16, u16) + Clone,
{
    fn idx(x: u16, y: u16) -> usize {
        (y * 24 + x) as usize
    }

    /// Returns true if something at (x, y) prevents the player from looking through that space
    fn sight_blocker(&self, x: u16, y: u16) -> bool {
        if x == 0 && y == 0 {
            false
        } else {
            let (xt, yt) = (self.transform)(x, y);
            let i = Self::idx(xt, yt);
            self.sight_blocker[i]
        }
    }

    /// Returns true if the player can look through the space (x, y)
    fn transparent(&mut self, x: u16, y: u16) -> bool {
        !self.sight_blocker(x, y) && self.visible(x, y)
    }

    /// Returns true if the player can see space (x, y). Coordinates are for an eighth of the
    /// players vision.
    /// For example, these are the coordinates for the bottom, right, upper quadrant with the
    /// player standing on 00:
    /// 00 10 20 30 40
    ///    11 21 31 41
    ///       22 32 42
    ///          33 43
    ///             44
    /// I had a lot of trouble finding the algorithm behind the right behavior just from observing
    /// game play, so this is basically me brute forcing it.
    fn visible(&mut self, x: u16, y: u16) -> bool {
        let key = if y == 0 {
            x
        } else if y == 1 {
            4 + x
        } else if y == 2 {
            7 + x
        } else if y == 3 {
            9 + x
        } else {
            10 + x
        };
        if x > 24 || y > 24 {
            panic!("{}, {}", x, y);
        }

        let key = key as usize;
        if let Some(result) = self.cache[key] {
            return result;
        }
        let result = if x == 0 && y == 0 {
            true
        } else if y == 0 {
            self.transparent(x - 1, 0)
        } else if x == y {
            self.transparent(x - 1, y - 1)
        } else if x == 2 && y == 1 {
            self.transparent(1, 0) || self.transparent(1, 1)
        } else if x == 3 && y == 1 {
            self.transparent(2, 0) || (self.transparent(2, 1) && self.transparent(1, 0))
        } else if x == 3 && y == 2 {
            self.transparent(2, 2)
                || (self.transparent(2, 1) && self.transparent(1, 1) && self.transparent(1, 0))
        } else if x == 4 && y == 1 {
            self.transparent(3, 0) || (self.transparent(3, 1) && self.transparent(2, 0))
        } else if x == 4 && y == 2 {
            self.transparent(1, 0)
                && self.transparent(1, 1)
                && self.transparent(2, 0)
                && self.transparent(2, 1)
                && (self.transparent(3, 1) || self.transparent(3, 2))
        } else if x == 4 && y == 3 {
            self.transparent(3, 3)
                || (self.transparent(3, 2)
                    && self.transparent(2, 2)
                    && self.transparent(2, 1)
                    && self.transparent(1, 0))
        } else {
            panic!("visible index out of bounds: {}, {}", x, y);
        };
        self.cache[key] = Some(result);
        result
    }
}

fn to_array(slice: &[bool]) -> [bool; 576] {
    let mut result = [false; 576];
    for (i, value) in slice.iter().enumerate() {
        result[i] = *value;
    }
    result
}
