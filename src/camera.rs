use crate::world::World;
use glam::{Mat4, Vec3};
use std::collections::HashSet;
use winit::keyboard::KeyCode;

const EYE_HEIGHT: f32 = 1.6;
const PLAYER_HALF_WIDTH: f32 = 0.3;
const PLAYER_HEIGHT: f32 = 1.8;

const MAX_WALK_SPEED: f32 = 6.5;
const GROUND_ACCEL: f32 = 28.0;  // Smooth acceleration on ground
const AIR_ACCEL: f32 = 14.0;     // Smooth acceleration in air
const FRICTION: f32 = 22.0;       // Smooth deceleration when stopping

const GRAVITY: f32 = 22.0;       // Smooth gravity
const TERMINAL_VELOCITY: f32 = -45.0;
const JUMP_VELOCITY: f32 = 8.5;  // Smooth jump impulse
const MOUSE_SENSITIVITY: f32 = 0.0025;
const VOID_Y_THRESHOLD: f32 = -20.0; // Instant damage / death void threshold

pub struct Camera {
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub respawn_pos: Vec3,
}

impl Camera {
    pub fn new(spawn_feet_position: Vec3) -> Self {
        Self {
            position: spawn_feet_position,
            velocity: Vec3::ZERO,
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: -0.3,
            on_ground: false,
            respawn_pos: spawn_feet_position,
        }
    }

    pub fn eye_position(&self) -> Vec3 {
        self.position + Vec3::new(0.0, EYE_HEIGHT, 0.0)
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    fn flat_forward(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin()).normalize()
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let eye = self.eye_position();
        let forward = self.forward();
        let view = Mat4::look_at_rh(eye, eye + forward, Vec3::Y);
        let proj = Mat4::perspective_rh(60f32.to_radians(), aspect, 0.1, 500.0);
        proj * view
    }

    pub fn apply_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * MOUSE_SENSITIVITY;
        self.pitch = (self.pitch - dy * MOUSE_SENSITIVITY).clamp(-1.54, 1.54);
    }

    pub fn update(&mut self, keys: &HashSet<KeyCode>, world: &World, dt: f32) {
        // --- 1. Horizontal Movement (Smooth Lerp Acceleration & Friction) ---
        let flat_forward = self.flat_forward();
        let flat_right = flat_forward.cross(Vec3::Y).normalize();

        let mut wish_dir = Vec3::ZERO;
        if keys.contains(&KeyCode::KeyW) { wish_dir += flat_forward; }
        if keys.contains(&KeyCode::KeyS) { wish_dir -= flat_forward; }
        if keys.contains(&KeyCode::KeyD) { wish_dir += flat_right; }
        if keys.contains(&KeyCode::KeyA) { wish_dir -= flat_right; }

        if wish_dir.length_squared() > 0.0 {
            wish_dir = wish_dir.normalize();
            let target_vel = wish_dir * MAX_WALK_SPEED;
            let accel_rate = if self.on_ground { GROUND_ACCEL } else { AIR_ACCEL };
            
            self.velocity.x += (target_vel.x - self.velocity.x) * (accel_rate * dt).min(1.0);
            self.velocity.z += (target_vel.z - self.velocity.z) * (accel_rate * dt).min(1.0);
        } else if self.on_ground {
            // Apply smooth friction when stopping on ground
            let decel = (FRICTION * dt).min(1.0);
            self.velocity.x *= 1.0 - decel;
            self.velocity.z *= 1.0 - decel;
        }

        // --- 2. Vertical Movement & Smooth Gravity ---
        self.velocity.y = (self.velocity.y - GRAVITY * dt).max(TERMINAL_VELOCITY);
        if keys.contains(&KeyCode::Space) && self.on_ground {
            self.velocity.y = JUMP_VELOCITY;
            self.on_ground = false;
        }

        // --- 3. Collision Resolution ---
        self.move_and_collide(world, dt);

        // --- 4. Instant Damage to Void Check ---
        if self.position.y < VOID_Y_THRESHOLD {
            println!("[VOID DAMAGE]: Instant void death! Falling below Y={:.1}. Respawning...", VOID_Y_THRESHOLD);
            self.position = self.respawn_pos;
            self.velocity = Vec3::ZERO;
            self.on_ground = false;
        }
    }

    fn move_and_collide(&mut self, world: &World, dt: f32) {
        let delta = self.velocity * dt;

        self.position.x += delta.x;
        if self.is_colliding(world) {
            self.position.x -= delta.x;
            self.velocity.x = 0.0;
        }

        self.position.z += delta.z;
        if self.is_colliding(world) {
            self.position.z -= delta.z;
            self.velocity.z = 0.0;
        }

        self.on_ground = false;
        self.position.y += delta.y;
        if self.is_colliding(world) {
            self.position.y -= delta.y;
            if self.velocity.y < 0.0 {
                self.on_ground = true;
            }
            self.velocity.y = 0.0;
        }
    }

    fn is_colliding(&self, world: &World) -> bool {
        let min = self.position - Vec3::new(PLAYER_HALF_WIDTH, 0.0, PLAYER_HALF_WIDTH);
        let max = self.position + Vec3::new(PLAYER_HALF_WIDTH, PLAYER_HEIGHT, PLAYER_HALF_WIDTH);

        let (min_x, max_x) = (min.x.floor() as i32, max.x.floor() as i32);
        let (min_y, max_y) = (min.y.floor() as i32, max.y.floor() as i32);
        let (min_z, max_z) = (min.z.floor() as i32, max.z.floor() as i32);

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                for z in min_z..=max_z {
                    if world.is_solid_at(x, y, z) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
