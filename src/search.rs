//! A* search
use crate::stage::Occupancy;
use std::{cmp::Ordering, collections::BinaryHeap};

const MOVES: &[(f64, f64)] = &[
    (1.0, 0.0),
    (0.0, 1.0),
    (1.0, 1.0),
    (1.0, -1.0),
    (-1.0, 1.0),
    (-1.0, 0.0),
    (0.0, -1.0),
    (-1.0, -1.0),
];

/// Returns the next step in the shortest path to hit the target
/// result = (x, y, success). (x, y) will be the next step along the
/// path to the target (x1, y1). Even if there is no full path to the
/// target a step will be returned that minimizes the distance do the target.
/// If there is no full path success will be false
pub(crate) fn search_path(
    occupancy: &Occupancy,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> (f64, f64, bool) {
    let mut g_score = Grid::new();
    g_score.set(x0, y0, h(x0, y0, x1, y1));
    let mut open: BinaryHeap<Node> = BinaryHeap::new();
    open.push(Node {
        x: x0,
        y: y0,
        cost: 0.0,
    });
    let mut iter = 0;
    while iter < 50 {
        let Some(node) = open.pop() else {
            break;
        };
        if node.x == x1 && node.y == y1 {
            let (x, y) = get_step(x0, y0, x1, y1, &g_score);
            return (x, y, true);
        }
        expand_node(&node, &mut open, occupancy, &mut g_score, x1, y1);
        iter += 1;
    }
    if let Some((new_x1, new_y1)) = g_score.closest(x1, y1) {
        let (x, y) = get_step(x0, y0, new_x1, new_y1, &g_score);
        return (x, y, false);
    }
    (x0, y0, false)
}

/// Used by search_path. Explores a node in the graph by visiting all possible
/// child nodes adding newly explored nodes to the open set and updates the
/// cost function g_score
fn expand_node(
    node: &Node,
    open: &mut BinaryHeap<Node>,
    occupancy: &Occupancy,
    g_score: &mut Grid,
    target_x: f64,
    target_y: f64,
) {
    for (dx, dy) in MOVES {
        let (x, y) = (node.x + dx, node.y + dy);

        if x > 23.0 || x < 0.0 || y > 23.0 || y < 0.0 {
            continue;
        }
        let cost = node.cost
            + ((node.x - x).powf(2.0) + (node.y - y).powf(2.0)).sqrt()
            + h(x, y, target_x, target_y);

        if let Some(old_cost) = g_score.get(x, y) {
            if cost >= old_cost {
                continue;
            }
        }
        if (x, y) == (target_x, target_y) || !occupancy.occupied(x, y) {
            g_score.set(x, y, cost);
            open.push(Node { x, y, cost });
        }
    }
}

/// Given a initial and end point and a cost function grid return the first
/// step optimal path from the initial to the end point
fn get_step(x0: f64, y0: f64, x1: f64, y1: f64, g_score: &Grid) -> (f64, f64) {
    if (x0, y0) == (x1, y1) {
        return (x0, y0);
    }
    let (mut x, mut y) = (x1, y1);
    loop {
        let mut next_x = 0.0;
        let mut next_y = 0.0;
        let mut cost = f64::MAX;

        for (dx, dy) in MOVES {
            let (xp, yp) = (x + dx, y + dy);
            if xp > 23.0 || xp < 0.0 || yp > 23.0 || yp < 0.0 {
                continue;
            }
            if (xp, yp) == (x0, y0) {
                return (x, y);
            }
            let Some(cost_p) = g_score.get(xp, yp) else {
                continue;
            };
            if cost_p < cost {
                next_x = xp;
                next_y = yp;
                cost = cost_p;
            }
        }
        x = next_x;
        y = next_y;
    }
}

/// A 24 x 24 grid
struct Grid {
    cells: [Option<f64>; 576],
}

impl Grid {
    fn new() -> Self {
        Self { cells: [None; 576] }
    }

    fn idx(x: f64, y: f64) -> usize {
        let xp = x.floor() as usize;
        let yp = y.floor() as usize;
        yp * 24 + xp
    }

    fn set(&mut self, x: f64, y: f64, value: f64) {
        let i = Self::idx(x, y);
        self.cells[i] = Some(value);
    }

    fn get(&self, x: f64, y: f64) -> Option<f64> {
        let i = Self::idx(x, y);
        self.cells[i]
    }

    // Returns the closest non-None point to the target
    fn closest(&self, target_x: f64, target_y: f64) -> Option<(f64, f64)> {
        let mut best_cost = f64::MAX;
        let mut best = None;
        for x in 0..23 {
            for y in 0..23 {
                let x = x as f64;
                let y = y as f64;
                if self.get(x, y).is_none() {
                    continue;
                }
                let cost = ((x - target_x).powf(2.0) + (y - target_y).powf(2.0)).sqrt();

                if cost < best_cost {
                    best_cost = cost;
                    best = Some((x, y));
                }
            }
        }
        best
    }
}

struct Node {
    x: f64,
    y: f64,
    cost: f64,
}

impl PartialEq for Node {
    fn eq(&self, other: &Node) -> bool {
        self.cost == other.cost
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Node) -> Option<Ordering> {
        if self.cost > other.cost {
            Some(Ordering::Less)
        } else if self.cost < other.cost {
            Some(Ordering::Greater)
        } else {
            Some(Ordering::Equal)
        }
    }
}

impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, other: &Node) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

/// A cost hueristic frox (x0, y0) to (x1, y1)
fn h(x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    ((x0 - x1).powf(2.0) + (y0 - y1).powf(2.0)).sqrt()
}
