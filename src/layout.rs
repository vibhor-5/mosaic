//! Layout engine module for Mosaic.
//!
//! Implements window tiling algorithms: BSP (Binary Space Partitioning), Monocle, and Master-Stack.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

pub type WindowId = u32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    Bsp,
    Monocle,
    MasterStack { master_ratio: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

pub enum BspNode {
    Split {
        direction: SplitDirection,
        ratio: f64,
        first: Box<BspNode>,
        second: Box<BspNode>,
    },
    Leaf {
        window_id: WindowId,
    },
    Empty,
}

impl BspNode {
    pub fn new_empty() -> Self {
        BspNode::Empty
    }

    pub fn insert(&mut self, window_id: WindowId) -> bool {
        match self {
            BspNode::Empty => {
                *self = BspNode::Leaf { window_id };
                true
            }
            BspNode::Leaf { window_id: existing_id } => {
                let existing = *existing_id;
                *self = BspNode::Split {
                    direction: SplitDirection::Horizontal, // Default, can be refined based on aspect ratio
                    ratio: 0.5,
                    first: Box::new(BspNode::Leaf { window_id: existing }),
                    second: Box::new(BspNode::Leaf { window_id }),
                };
                true
            }
            BspNode::Split { first, second, .. } => {
                if first.insert(window_id) {
                    true
                } else {
                    second.insert(window_id)
                }
            }
        }
    }

    pub fn remove(&mut self, window_id: WindowId) -> bool {
        let mut replace_with = None;
        let removed = match self {
            BspNode::Empty => false,
            BspNode::Leaf { window_id: id } => {
                if *id == window_id {
                    *self = BspNode::Empty;
                    true
                } else {
                    false
                }
            }
            BspNode::Split { first, second, .. } => {
                if first.remove(window_id) {
                    if let BspNode::Empty = **first {
                        replace_with = Some(std::mem::replace(&mut **second, BspNode::Empty));
                    }
                    true
                } else if second.remove(window_id) {
                    if let BspNode::Empty = **second {
                        replace_with = Some(std::mem::replace(&mut **first, BspNode::Empty));
                    }
                    true
                } else {
                    false
                }
            }
        };

        if let Some(node) = replace_with {
            *self = node;
        }
        removed
    }

    pub fn find(&self, window_id: WindowId) -> Option<&BspNode> {
        match self {
            BspNode::Empty => None,
            BspNode::Leaf { window_id: id } => {
                if *id == window_id {
                    Some(self)
                } else {
                    None
                }
            }
            BspNode::Split { first, second, .. } => {
                first.find(window_id).or_else(|| second.find(window_id))
            }
        }
    }

    pub fn find_mut(&mut self, window_id: WindowId) -> Option<&mut BspNode> {
        match self {
            BspNode::Empty => None,
            BspNode::Leaf { window_id: id } => {
                if *id == window_id {
                    Some(self)
                } else {
                    None
                }
            }
            BspNode::Split { first, second, .. } => {
                if first.find(window_id).is_some() {
                    first.find_mut(window_id)
                } else {
                    second.find_mut(window_id)
                }
            }
        }
    }

    pub fn swap(&mut self, a: WindowId, b: WindowId) -> bool {
        let mut a_ptr: Option<*mut WindowId> = None;
        let mut b_ptr: Option<*mut WindowId> = None;

        fn find_ptrs(node: &mut BspNode, a: WindowId, b: WindowId, a_ptr: &mut Option<*mut WindowId>, b_ptr: &mut Option<*mut WindowId>) {
            match node {
                BspNode::Leaf { window_id } => {
                    if *window_id == a {
                        *a_ptr = Some(window_id as *mut WindowId);
                    } else if *window_id == b {
                        *b_ptr = Some(window_id as *mut WindowId);
                    }
                }
                BspNode::Split { first, second, .. } => {
                    find_ptrs(first, a, b, a_ptr, b_ptr);
                    find_ptrs(second, a, b, a_ptr, b_ptr);
                }
                _ => {}
            }
        }

        find_ptrs(self, a, b, &mut a_ptr, &mut b_ptr);

        if let (Some(ap), Some(bp)) = (a_ptr, b_ptr) {
            unsafe {
                std::ptr::swap(ap, bp);
            }
            true
        } else {
            false
        }
    }

    pub fn compute_rects(&self, region: Rect, gap: f64) -> Vec<(WindowId, Rect)> {
        let mut rects = Vec::new();
        self.compute_rects_impl(region, gap, &mut rects);
        rects
    }

    fn compute_rects_impl(&self, region: Rect, gap: f64, rects: &mut Vec<(WindowId, Rect)>) {
        match self {
            BspNode::Empty => {}
            BspNode::Leaf { window_id } => {
                rects.push((*window_id, region));
            }
            BspNode::Split { direction, ratio, first, second } => {
                let mut first_region = region;
                let mut second_region = region;

                match direction {
                    SplitDirection::Horizontal => {
                        let total_width = region.width - gap;
                        let first_width = (total_width * ratio).floor();
                        first_region.width = first_width;
                        second_region.x += first_width + gap;
                        second_region.width = total_width - first_width;
                    }
                    SplitDirection::Vertical => {
                        let total_height = region.height - gap;
                        let first_height = (total_height * ratio).floor();
                        first_region.height = first_height;
                        second_region.y += first_height + gap;
                        second_region.height = total_height - first_height;
                    }
                }

                first.compute_rects_impl(first_region, gap, rects);
                second.compute_rects_impl(second_region, gap, rects);
            }
        }
    }

    pub fn rotate(&mut self) {
        match self {
            BspNode::Split { direction, first, second, .. } => {
                *direction = match direction {
                    SplitDirection::Horizontal => SplitDirection::Vertical,
                    SplitDirection::Vertical => SplitDirection::Horizontal,
                };
                first.rotate();
                second.rotate();
            }
            _ => {}
        }
    }

    pub fn equalize(&mut self) {
        match self {
            BspNode::Split { ratio, first, second, .. } => {
                *ratio = 0.5;
                first.equalize();
                second.equalize();
            }
            _ => {}
        }
    }
    
    pub fn window_count(&self) -> usize {
        match self {
            BspNode::Empty => 0,
            BspNode::Leaf { .. } => 1,
            BspNode::Split { first, second, .. } => first.window_count() + second.window_count()
        }
    }

    pub fn windows(&self) -> Vec<WindowId> {
        let mut ids = Vec::new();
        self.collect_windows(&mut ids);
        ids
    }

    fn collect_windows(&self, ids: &mut Vec<WindowId>) {
        match self {
            BspNode::Empty => {}
            BspNode::Leaf { window_id } => ids.push(*window_id),
            BspNode::Split { first, second, .. } => {
                first.collect_windows(ids);
                second.collect_windows(ids);
            }
        }
    }
}

pub struct LayoutEngine {
    mode: LayoutMode,
    tree: BspNode,
    gap_inner: f64,
    gap_outer: f64,
    windows: Vec<WindowId>, // Keep a flat list for monocle/master-stack order
}

impl LayoutEngine {
    pub fn new(mode: LayoutMode, gap_inner: f64, gap_outer: f64) -> Self {
        Self {
            mode,
            tree: BspNode::new_empty(),
            gap_inner,
            gap_outer,
            windows: Vec::new(),
        }
    }

    pub fn add_window(&mut self, window_id: WindowId) {
        if !self.windows.contains(&window_id) {
            self.windows.push(window_id);
            self.tree.insert(window_id);
        }
    }

    pub fn remove_window(&mut self, window_id: WindowId) {
        self.windows.retain(|&id| id != window_id);
        self.tree.remove(window_id);
    }
    
    pub fn has_window(&self, window_id: WindowId) -> bool {
        self.windows.contains(&window_id)
    }

    pub fn compute_layout(&self, screen: Rect) -> Vec<(WindowId, Rect)> {
        if self.windows.is_empty() {
            return Vec::new();
        }

        let region = Rect {
            x: screen.x + self.gap_outer,
            y: screen.y + self.gap_outer,
            width: screen.width - self.gap_outer * 2.0,
            height: screen.height - self.gap_outer * 2.0,
        };

        match self.mode {
            LayoutMode::Bsp => self.tree.compute_rects(region, self.gap_inner),
            LayoutMode::Monocle => {
                self.windows.iter().map(|&id| (id, region)).collect()
            }
            LayoutMode::MasterStack { master_ratio } => {
                if self.windows.len() == 1 {
                    vec![(self.windows[0], region)]
                } else {
                    let mut rects = Vec::new();
                    let master_width = (region.width * master_ratio) - (self.gap_inner / 2.0);
                    let stack_width = region.width - master_width - self.gap_inner;
                    
                    // Master window
                    let master_rect = Rect {
                        x: region.x,
                        y: region.y,
                        width: master_width,
                        height: region.height,
                    };
                    rects.push((self.windows[0], master_rect));

                    // Stack windows
                    let stack_count = self.windows.len() - 1;
                    let total_gaps = self.gap_inner * (stack_count - 1) as f64;
                    let stack_height = (region.height - total_gaps) / stack_count as f64;

                    for (i, &window_id) in self.windows.iter().skip(1).enumerate() {
                        let rect = Rect {
                            x: region.x + master_width + self.gap_inner,
                            y: region.y + (stack_height + self.gap_inner) * i as f64,
                            width: stack_width,
                            height: stack_height,
                        };
                        rects.push((window_id, rect));
                    }
                    rects
                }
            }
        }
    }

    pub fn swap_windows(&mut self, a: WindowId, b: WindowId) {
        self.tree.swap(a, b);
        let a_idx = self.windows.iter().position(|&x| x == a);
        let b_idx = self.windows.iter().position(|&x| x == b);
        if let (Some(ai), Some(bi)) = (a_idx, b_idx) {
            self.windows.swap(ai, bi);
        }
    }

    pub fn set_mode(&mut self, mode: LayoutMode) {
        self.mode = mode;
    }

    pub fn rotate_tree(&mut self) {
        self.tree.rotate();
    }

    pub fn equalize_tree(&mut self) {
        self.tree.equalize();
    }

    pub fn resize_window(&mut self, _window_id: WindowId, _direction: SplitDirection, _delta: f64) {
        // Complex to implement without knowing tree structure cleanly, skipping for now
    }

    pub fn focus_next(&self, current: WindowId) -> Option<WindowId> {
        let idx = self.windows.iter().position(|&x| x == current)?;
        let next_idx = (idx + 1) % self.windows.len();
        Some(self.windows[next_idx])
    }

    pub fn focus_prev(&self, current: WindowId) -> Option<WindowId> {
        let idx = self.windows.iter().position(|&x| x == current)?;
        let next_idx = if idx == 0 { self.windows.len() - 1 } else { idx - 1 };
        Some(self.windows[next_idx])
    }

    pub fn focus_direction(&self, current: WindowId, _dir: Direction) -> Option<WindowId> {
        // Fallback to simple next/prev for now since directional focus in BSP requires spatial awareness
        self.focus_next(current)
    }
}
