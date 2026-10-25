//! A 3D player model for the account page, drawn as a textured egui mesh.
//!
//! The model is Minecraft's: boxes measured in skin pixels (the player is 32
//! tall), each face mapped onto the standard 64x64 skin layout, with the
//! second layer (hat, jacket, sleeves, trousers) as slightly larger boxes
//! around the first. Every frame each face is posed, rotated, projected and
//! back-face culled, and the visible faces are painted far to near. Around
//! two hundred triangles: nothing a GPU notices.

use std::f32::consts::PI;

use eframe::egui::{self, Color32, Mesh, Painter, Pos2, Rect, TextureId, epaint::Vertex, pos2};

/// How the model stands this frame. Angles in radians.
#[derive(Clone, Copy, Default)]
pub struct Pose {
    /// Turn of the whole model around the vertical axis (0 = facing you).
    pub yaw: f32,
    /// Sideways lean of the whole model.
    pub roll: f32,
    /// Height above its resting place, in skin pixels.
    pub lift: f32,
    /// Sideways offset from the middle of the box, in skin pixels.
    pub shift: f32,
    pub head_yaw: f32,
    pub head_pitch: f32,
    /// Arm swing forward/back (x) and outward (z), right then left.
    pub right_arm: (f32, f32),
    pub left_arm: (f32, f32),
    pub right_leg: f32,
    pub left_leg: f32,
    /// 0..1, for fading in.
    pub alpha: f32,
}

#[derive(Clone, Copy)]
struct Part {
    /// Box corner and size, in model space (y up, the player faces +z).
    min: [f32; 3],
    size: [f32; 3],
    /// Top-left of the box's area on the skin, and its size in texels.
    uv: (f32, f32),
    texels: [f32; 3],
    mirror: bool,
    /// Rotation centre and angles (x, y, z) applied before the whole-body turn.
    pivot: [f32; 3],
    rot: [f32; 3],
}

impl Part {
    fn new(min: [f32; 3], size: [f32; 3], uv: (f32, f32)) -> Self {
        Part { min, size, uv, texels: size, mirror: false, pivot: [0.0; 3], rot: [0.0; 3] }
    }

    /// The same box grown by `by` on every side (the second skin layer).
    fn grown(self, by: f32, uv: (f32, f32)) -> Self {
        Part {
            min: [self.min[0] - by, self.min[1] - by, self.min[2] - by],
            size: [self.size[0] + by * 2.0, self.size[1] + by * 2.0, self.size[2] + by * 2.0],
            uv,
            ..self
        }
    }

    fn pivoted(self, pivot: [f32; 3], rot: [f32; 3]) -> Self {
        Part { pivot, rot, ..self }
    }
}

fn rotate(p: [f32; 3], pivot: [f32; 3], r: [f32; 3]) -> [f32; 3] {
    let (mut x, mut y, mut z) = (p[0] - pivot[0], p[1] - pivot[1], p[2] - pivot[2]);
    let (s, c) = r[0].sin_cos();
    (y, z) = (y * c - z * s, y * s + z * c);
    let (s, c) = r[1].sin_cos();
    (x, z) = (x * c + z * s, -x * s + z * c);
    let (s, c) = r[2].sin_cos();
    (x, y) = (x * c - y * s, x * s + y * c);
    [x + pivot[0], y + pivot[1], z + pivot[2]]
}

/// The model's boxes for this pose.
fn parts(pose: &Pose, slim: bool, legacy: bool) -> Vec<Part> {
    let arm_w = if slim { 3.0 } else { 4.0 };
    let head = Part::new([-4.0, 24.0, -4.0], [8.0, 8.0, 8.0], (0.0, 0.0)).pivoted([0.0, 24.0, 0.0], [pose.head_pitch, pose.head_yaw, 0.0]);
    let body = Part::new([-4.0, 12.0, -2.0], [8.0, 12.0, 4.0], (16.0, 16.0));
    // The player's right side is towards -x (your left as it faces you).
    let right_arm = Part::new([-4.0 - arm_w, 12.0, -2.0], [arm_w, 12.0, 4.0], (40.0, 16.0))
        .pivoted([-5.0, 22.0, 0.0], [pose.right_arm.0, 0.0, -pose.right_arm.1]);
    let right_leg = Part::new([-4.0, 0.0, -2.0], [4.0, 12.0, 4.0], (0.0, 16.0)).pivoted([-2.0, 12.0, 0.0], [pose.right_leg, 0.0, 0.0]);
    let left_arm_base = Part::new([4.0, 12.0, -2.0], [arm_w, 12.0, 4.0], (32.0, 48.0))
        .pivoted([5.0, 22.0, 0.0], [pose.left_arm.0, 0.0, pose.left_arm.1]);
    let left_leg_base = Part::new([0.0, 0.0, -2.0], [4.0, 12.0, 4.0], (16.0, 48.0)).pivoted([2.0, 12.0, 0.0], [pose.left_leg, 0.0, 0.0]);

    let mut out = vec![head, body, right_arm, right_leg];
    if legacy {
        // 64x32 skins have no left limbs: mirror the right ones.
        out.push(Part { uv: right_arm.uv, mirror: true, ..left_arm_base });
        out.push(Part { uv: right_leg.uv, mirror: true, ..left_leg_base });
        out.push(head.grown(0.5, (32.0, 0.0)));
    } else {
        out.push(left_arm_base);
        out.push(left_leg_base);
        out.push(head.grown(0.5, (32.0, 0.0)));
        out.push(body.grown(0.25, (16.0, 32.0)));
        out.push(right_arm.grown(0.25, (40.0, 32.0)));
        out.push(left_arm_base.grown(0.25, (48.0, 48.0)));
        out.push(right_leg.grown(0.25, (0.0, 32.0)));
        out.push(left_leg_base.grown(0.25, (0.0, 48.0)));
    }
    out
}

/// One face: corners (top-left, top-right, bottom-right, bottom-left as seen
/// from outside) and its texel rectangle (u0, v0, u1, v1).
fn faces(p: &Part) -> [([[f32; 3]; 4], [f32; 4]); 6] {
    let [x0, y0, z0] = p.min;
    let (x1, y1, z1) = (x0 + p.size[0], y0 + p.size[1], z0 + p.size[2]);
    let [w, h, d] = p.texels;
    let (u, v) = p.uv;
    let rect = |u0: f32, v0: f32, uw: f32, vh: f32| [u0, v0, u0 + uw, v0 + vh];
    // Texture areas, laid out as in every Minecraft skin.
    let mut right = rect(u, v + d, d, h);
    let front = rect(u + d, v + d, w, h);
    let mut left = rect(u + d + w, v + d, d, h);
    let back = rect(u + 2.0 * d + w, v + d, w, h);
    let top = rect(u + d, v, w, d);
    let bottom = rect(u + d + w, v, w, d);
    let mut list = [
        ([[x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [x0, y0, z1]], front),
        ([[x1, y1, z0], [x0, y1, z0], [x0, y0, z0], [x1, y0, z0]], back),
        ([[x0, y1, z0], [x0, y1, z1], [x0, y0, z1], [x0, y0, z0]], right),
        ([[x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [x1, y0, z1]], left),
        ([[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]], top),
        ([[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]], bottom),
    ];
    if p.mirror {
        // A mirrored box swaps its side textures and flips every face.
        std::mem::swap(&mut right, &mut left);
        list[2].1 = right;
        list[3].1 = left;
        for f in &mut list {
            f.1 = [f.1[2], f.1[1], f.1[0], f.1[3]];
        }
    }
    list
}

/// Draws the model centred in `rect`. `texture` is the 64x64 skin with
/// nearest filtering; without one the model is drawn in flat greys.
pub fn paint(painter: &Painter, rect: Rect, texture: Option<TextureId>, slim: bool, legacy: bool, pose: &Pose) {
    // Seen from a little above, like a player looking at it.
    const VIEW_PITCH: f32 = 0.18;
    const FOCAL: f32 = 140.0;
    let scale = rect.height() / 40.0;
    let centre = pos2(rect.center().x, rect.center().y + 2.0 * scale);
    let light = normalize([-0.35, 0.55, 0.75]);
    let alpha = pose.alpha.clamp(0.0, 1.0);

    struct Quad {
        depth: f32,
        pos: [Pos2; 4],
        uv: [f32; 4],
        shade: f32,
    }
    let mut quads = Vec::with_capacity(96);
    for part in parts(pose, slim, legacy) {
        for (corners, uv) in faces(&part) {
            let world = corners.map(|c| {
                let c = rotate(c, part.pivot, part.rot);
                // Whole body: lean, turn, then lift; then the camera's tilt.
                let c = rotate(c, [0.0, 0.0, 0.0], [0.0, pose.yaw, pose.roll]);
                let c = [c[0] + pose.shift, c[1] + pose.lift - 16.0, c[2]];
                rotate(c, [0.0; 3], [VIEW_PITCH, 0.0, 0.0])
            });
            let pos = world.map(|c| {
                let k = FOCAL / (FOCAL - c[2]);
                pos2(centre.x + c[0] * scale * k, centre.y - c[1] * scale * k)
            });
            // Back-face culling: visible faces wind clockwise on screen.
            let cross = (pos[1].x - pos[0].x) * (pos[2].y - pos[0].y) - (pos[1].y - pos[0].y) * (pos[2].x - pos[0].x);
            if cross <= 0.0 {
                continue;
            }
            // Outward normal (down x across, for these corner orders).
            let n = normalize(cross3(sub(world[3], world[0]), sub(world[1], world[0])));
            let shade = 0.62 + 0.38 * dot(n, light).max(0.0);
            let depth = world.iter().map(|c| c[2]).sum::<f32>() / 4.0;
            quads.push(Quad { depth, pos, uv, shade });
        }
    }
    quads.sort_by(|a, b| a.depth.total_cmp(&b.depth));

    let mut mesh = Mesh::with_texture(texture.unwrap_or_default());
    // Stay inside each face's texels so neighbours never bleed in.
    const INSET: f32 = 0.02;
    for q in quads {
        let (mut u0, v0, mut u1, v1) = (q.uv[0], q.uv[1], q.uv[2], q.uv[3]);
        let sign = if u1 >= u0 { 1.0 } else { -1.0 };
        u0 += INSET * sign;
        u1 -= INSET * sign;
        let (v0, v1) = (v0 + INSET, v1 - INSET);
        let uv = [pos2(u0, v0), pos2(u1, v0), pos2(u1, v1), pos2(u0, v1)].map(|p| pos2(p.x / 64.0, p.y / 64.0));
        let s = (q.shade * 255.0) as u8;
        let color = match texture {
            Some(_) => Color32::from_rgba_premultiplied(s, s, s, 255),
            None => Color32::from_gray((q.shade * 70.0) as u8),
        }
        .gamma_multiply(alpha);
        let base = mesh.vertices.len() as u32;
        for (pos, uv) in q.pos.into_iter().zip(uv) {
            mesh.vertices.push(Vertex { pos, uv: if texture.is_some() { uv } else { egui::epaint::WHITE_UV }, color });
        }
        mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// The account page's model: drops in when shown, then idles. Drag to turn it.
pub struct SkinView {
    shown: std::time::Instant,
    /// Turn added by dragging, in radians.
    turned: f32,
    dragged: bool,
}

impl Default for SkinView {
    fn default() -> Self {
        SkinView { shown: std::time::Instant::now(), turned: 0.0, dragged: false }
    }
}

/// Walking in from the right edge of the box to the middle.
const WALK: f32 = 1.35;
/// Turning to face the viewer, head and body together. It starts during
/// the last step, so stopping and turning are one motion.
const TURN: f32 = 0.5;
const TURN_AT: f32 = WALK - 0.2;
/// Where the walk starts: past the right edge of the box, in skin pixels.
const WALK_FROM: f32 = 34.0;
/// The resting turn: a three-quarter view, like a player looking at you.
const REST_YAW: f32 = -0.42;
/// Leg swing per skin pixel walked: a 12 px leg swinging ±0.6 rad covers
/// about 13.5 px per stride, so the feet don't slide.
const STRIDE: f32 = PI / 13.5;
/// Walking at full speed for this share of the walk, then slowing to a stop.
const CRUISE: f32 = 0.6;

/// Share (0..1) of the distance covered at `p` (0..1) through the walk:
/// constant speed, then slowing evenly to a stop.
fn walked(p: f32) -> f32 {
    let total = CRUISE + (1.0 - CRUISE) / 2.0;
    let d = if p < CRUISE {
        p
    } else {
        let q = p - CRUISE;
        CRUISE + q - q * q / (2.0 * (1.0 - CRUISE))
    };
    d / total
}

/// Walking speed at `p`, 1 at full speed.
fn speed(p: f32) -> f32 {
    if p < CRUISE { 1.0 } else { ((1.0 - p) / (1.0 - CRUISE)).max(0.0) }
}

fn ease_in_out(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * p * (3.0 - 2.0 * p)
}

impl SkinView {
    /// Plays the entrance again (when the page opens or the account changes).
    pub fn restart(&mut self) {
        *self = SkinView::default();
    }

    /// True while something on it is moving.
    pub fn animating(&self) -> bool {
        true
    }

    pub fn drag(&mut self, dx: f32) {
        self.turned += dx * 0.012;
        self.dragged = true;
    }

    pub fn pose(&self) -> Pose {
        let t = self.shown.elapsed().as_secs_f32();

        // Walking in, side-on, facing where it's going (-x is yaw -90°).
        let wp = (t / WALK).min(1.0);
        let distance = walked(wp) * WALK_FROM.abs();
        let stride = speed(wp);
        let phase = distance * STRIDE;
        let swing = phase.sin() * 0.6 * stride;

        // Turning to face you as it stops.
        let tp = ease_in_out((t - TURN_AT) / TURN);
        let body_yaw = -PI / 2.0 + (REST_YAW + PI / 2.0) * tp;

        let idle = (t - TURN_AT - TURN).max(0.0);
        let sway = if self.dragged { 0.0 } else { 0.10 * (idle * 0.45).sin() * ease_in_out(idle) };
        Pose {
            yaw: body_yaw + sway + self.turned,
            roll: 0.0,
            // A slight bob, twice per stride.
            lift: phase.sin().abs() * 0.5 * stride,
            shift: WALK_FROM - distance,
            head_yaw: 0.10 * (idle * 0.6).sin() * ease_in_out(idle),
            head_pitch: 0.05 * (idle * 0.8).sin() - 0.05,
            // Arms swing against the legs while walking, then hang and breathe.
            right_arm: (swing + 0.06 * (idle * 1.1).sin(), 0.06 + 0.03 * (idle * 0.9).sin()),
            left_arm: (-swing - 0.06 * (idle * 1.1).sin(), 0.06 + 0.03 * (idle * 0.9 + PI).sin()),
            right_leg: -swing,
            left_leg: swing,
            alpha: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_of_head_maps_to_the_face() {
        let head = Part::new([-4.0, 24.0, -4.0], [8.0, 8.0, 8.0], (0.0, 0.0));
        let f = faces(&head);
        assert_eq!(f[0].1, [8.0, 8.0, 16.0, 16.0]); // front = face
        assert_eq!(f[4].1, [8.0, 0.0, 16.0, 8.0]); // top
    }

    #[test]
    fn mirrored_box_swaps_and_flips() {
        let arm = Part { mirror: true, ..Part::new([4.0, 12.0, -2.0], [4.0, 12.0, 4.0], (40.0, 16.0)) };
        let f = faces(&arm);
        assert_eq!(f[0].1, [48.0, 20.0, 44.0, 32.0]); // front flipped
        assert_eq!(f[2].1, [52.0, 20.0, 48.0, 32.0]); // right side shows the left texture, flipped
    }

    #[test]
    fn front_normal_faces_the_viewer() {
        let head = Part::new([-4.0, 24.0, -4.0], [8.0, 8.0, 8.0], (0.0, 0.0));
        let c = faces(&head)[0].0;
        let n = normalize(cross3(sub(c[3], c[0]), sub(c[1], c[0])));
        assert!(n[2] > 0.99, "{n:?}");
    }

    #[test]
    fn model_has_both_layers_unless_legacy() {
        let pose = Pose::default();
        assert_eq!(parts(&pose, false, false).len(), 12);
        assert_eq!(parts(&pose, true, true).len(), 7);
    }

    #[test]
    fn walk_covers_the_distance_and_stops() {
        assert!(walked(0.0).abs() < 1e-6 && (walked(1.0) - 1.0).abs() < 1e-6);
        // Always moving forward.
        let mut last = 0.0;
        for i in 1..=100 {
            let w = walked(i as f32 / 100.0);
            assert!(w >= last);
            last = w;
        }
        assert_eq!(speed(1.0), 0.0);
    }

    #[test]
    fn entrance_settles_on_the_ground() {
        let view = SkinView { shown: std::time::Instant::now() - std::time::Duration::from_secs(5), turned: 0.0, dragged: true };
        let pose = view.pose();
        assert!(pose.lift.abs() < 1e-3 && pose.shift.abs() < 1e-3 && pose.right_leg.abs() < 1e-3);
        assert!((pose.yaw - REST_YAW).abs() < 1e-3);
    }
}
