use self::red_hat_boy_states::*;
use crate::{
    browser,
    engine::{self, Game, Image, KeyState, Point, Rect, Renderer},
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use web_sys::HtmlImageElement;

const HEIGHT: i16 = 600;
const HIGH_PLATFORM: i16 = 375;
const LOW_PLATFORM: i16 = 420;

pub struct Barrier {
    image: Image,
}

impl Barrier {
    pub fn new(image: Image) -> Self {
        Barrier { image }
    }
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub frame: SheetRect,
    pub sprite_source_size: SheetRect,
}

pub enum Event {
    Jump,
    KnockOut,
    Land(i16),
    Run,
    Slide,
    Update,
}

pub trait Obstacle {
    fn check_intersection(&self, boy: &mut RedHatBoy);
    fn draw(&self, renderer: &Renderer);
    fn draw_bounding_boxes(&self, renderer: &Renderer);
    fn move_horizontally(&mut self, x: i16);
    fn right(&self) -> i16;
}

impl Obstacle for Barrier {
    fn check_intersection(&self, boy: &mut RedHatBoy) {
        if boy.bounding_box().intersects(self.image.bounding_box()) {
            boy.knock_out();
        }
    }

    fn draw(&self, renderer: &Renderer) {
        self.image.draw(renderer);
        self.draw_bounding_boxes(renderer);
    }

    fn draw_bounding_boxes(&self, renderer: &Renderer) {
        renderer.draw_rect(self.image.bounding_box());
    }

    fn move_horizontally(&mut self, x: i16) {
        self.image.move_horizontally(x);
    }

    fn right(&self) -> i16 {
        self.image.right()
    }
}

impl Obstacle for Platform {
    fn check_intersection(&self, boy: &mut RedHatBoy) {
        if let Some(box_to_land_on) = self
            .bounding_boxes()
            .iter()
            .find(|&bounding_box| boy.bounding_box().intersects(bounding_box))
        {
            if boy.velocity_y() > 0 && boy.pos_y() < self.position.y {
                boy.land_on(box_to_land_on.y());
            } else {
                boy.knock_out();
            }
        }
    }

    fn draw(&self, renderer: &Renderer) {
        let platform = self
            .sheet
            .frames
            .get("13.png")
            .expect("13.png does not exist");

        renderer.draw_image(
            &self.image,
            &Rect::new_from_x_y(
                platform.frame.x.into(),
                platform.frame.y.into(),
                (platform.frame.w * 3).into(),
                platform.frame.h.into(),
            ),
            &self.destination_box(),
        );

        self.draw_bounding_boxes(renderer);
    }

    fn draw_bounding_boxes(&self, renderer: &Renderer) {
        for bounding_box in &self.bounding_boxes() {
            renderer.draw_rect(bounding_box);
        }
    }

    fn move_horizontally(&mut self, x: i16) {
        self.position.x += x
    }

    fn right(&self) -> i16 {
        self.bounding_boxes()
            .last()
            .unwrap_or(&Rect::default())
            .right()
    }
}

pub struct RedHatBoy {
    state_machine: RedHatBoyStateMachine,
    sprite_sheet: Sheet,
    image: HtmlImageElement,
}

impl RedHatBoy {
    fn bounding_box(&self) -> Rect {
        const X_OFFSET: i16 = 18;
        const Y_OFFSET: i16 = 14;
        const WIDTH_OFFSET: i16 = 28;

        Rect::new_from_x_y(
            self.destination_box().x() + X_OFFSET,
            self.destination_box().y() + Y_OFFSET,
            self.destination_box().width - WIDTH_OFFSET,
            self.destination_box().height - Y_OFFSET,
        )
    }

    fn current_sprite(&self) -> Option<&Cell> {
        self.sprite_sheet.frames.get(&self.frame_name())
    }

    fn destination_box(&self) -> Rect {
        let sprite = self.current_sprite().expect("Cell not found");

        Rect::new_from_x_y(
            (self.state_machine.context().position.x + sprite.sprite_source_size.x as i16).into(),
            (self.state_machine.context().position.y + sprite.sprite_source_size.y as i16).into(),
            sprite.sprite_source_size.w.into(),
            sprite.sprite_source_size.h.into(),
        )
    }

    fn draw(&self, renderer: &Renderer) {
        let sprite = self.current_sprite().expect("Cell not found");

        renderer.draw_image(
            &self.image,
            &Rect::new_from_x_y(
                sprite.frame.x.into(),
                sprite.frame.y.into(),
                sprite.frame.w.into(),
                sprite.frame.h.into(),
            ),
            &self.destination_box(),
        );

        renderer.draw_rect(&self.bounding_box());
    }

    fn frame_name(&self) -> String {
        format!(
            "{} ({}).png",
            self.state_machine.frame_name(),
            (self.state_machine.context().frame / 3) + 1
        )
    }

    fn jump(&mut self) {
        self.state_machine = self.state_machine.transition(Event::Jump);
    }

    fn knock_out(&mut self) {
        self.state_machine = self.state_machine.transition(Event::KnockOut);
    }

    fn land_on(&mut self, position: i16) {
        self.state_machine = self.state_machine.transition(Event::Land(position));
    }

    fn new(sheet: Sheet, image: HtmlImageElement) -> Self {
        RedHatBoy {
            state_machine: RedHatBoyStateMachine::Idle(RedHatBoyState::new()),
            sprite_sheet: sheet,
            image: image,
        }
    }

    fn pos_y(&self) -> i16 {
        self.state_machine.context().position.y
    }

    fn run_right(&mut self) {
        self.state_machine = self.state_machine.transition(Event::Run);
    }

    fn slide(&mut self) {
        self.state_machine = self.state_machine.transition(Event::Slide);
    }

    fn update(&mut self) {
        self.state_machine = self.state_machine.update();
    }

    fn velocity_y(&self) -> i16 {
        self.state_machine.context().velocity.y
    }

    fn walking_speed(&self) -> i16 {
        self.state_machine.context().velocity.x
    }
}

#[derive(Copy, Clone)]
enum RedHatBoyStateMachine {
    Falling(RedHatBoyState<Falling>),
    Idle(RedHatBoyState<Idle>),
    Jumping(RedHatBoyState<Jumping>),
    KnockedOut(RedHatBoyState<KnockedOut>),
    Running(RedHatBoyState<Running>),
    Sliding(RedHatBoyState<Sliding>),
}

impl RedHatBoyStateMachine {
    fn context(&self) -> &RedHatBoyContext {
        match self {
            RedHatBoyStateMachine::Falling(state) => state.context(),
            RedHatBoyStateMachine::Idle(state) => state.context(),
            RedHatBoyStateMachine::Jumping(state) => state.context(),
            RedHatBoyStateMachine::KnockedOut(state) => state.context(),
            RedHatBoyStateMachine::Running(state) => state.context(),
            RedHatBoyStateMachine::Sliding(state) => state.context(),
        }
    }

    fn frame_name(&self) -> &str {
        match self {
            RedHatBoyStateMachine::Falling(state) => state.frame_name(),
            RedHatBoyStateMachine::Idle(state) => state.frame_name(),
            RedHatBoyStateMachine::Jumping(state) => state.frame_name(),
            RedHatBoyStateMachine::KnockedOut(state) => state.frame_name(),
            RedHatBoyStateMachine::Running(state) => state.frame_name(),
            RedHatBoyStateMachine::Sliding(state) => state.frame_name(),
        }
    }

    fn transition(self, event: Event) -> Self {
        match (self, event) {
            (RedHatBoyStateMachine::Falling(state), Event::Update) => state.update().into(),

            (RedHatBoyStateMachine::Idle(state), Event::Run) => state.run().into(),
            (RedHatBoyStateMachine::Idle(state), Event::Update) => state.update().into(),

            (RedHatBoyStateMachine::Jumping(state), Event::KnockOut) => state.knock_out().into(),
            (RedHatBoyStateMachine::Jumping(state), Event::Land(position)) => {
                state.land_on(position).into()
            }
            (RedHatBoyStateMachine::Jumping(state), Event::Update) => state.update().into(),

            (RedHatBoyStateMachine::Running(state), Event::Jump) => state.jump().into(),
            (RedHatBoyStateMachine::Running(state), Event::Land(position)) => {
                state.land_on(position).into()
            }
            (RedHatBoyStateMachine::Running(state), Event::KnockOut) => state.knock_out().into(),
            (RedHatBoyStateMachine::Running(state), Event::Slide) => state.slide().into(),
            (RedHatBoyStateMachine::Running(state), Event::Update) => state.update().into(),

            (RedHatBoyStateMachine::Sliding(state), Event::KnockOut) => state.knock_out().into(),
            (RedHatBoyStateMachine::Sliding(state), Event::Land(position)) => {
                state.land_on(position).into()
            }
            (RedHatBoyStateMachine::Sliding(state), Event::Update) => state.update().into(),
            _ => self,
        }
    }

    fn update(self) -> Self {
        self.transition(Event::Update)
    }
}

impl From<FallingEndState> for RedHatBoyStateMachine {
    fn from(end_state: FallingEndState) -> Self {
        match end_state {
            FallingEndState::Falling(falling_state) => falling_state.into(),
            FallingEndState::KnockedOut(knockedout_state) => knockedout_state.into(),
        }
    }
}

impl From<JumpingEndState> for RedHatBoyStateMachine {
    fn from(end_state: JumpingEndState) -> Self {
        match end_state {
            JumpingEndState::Complete(running_state) => running_state.into(),
            JumpingEndState::Jumping(jumping_state) => jumping_state.into(),
        }
    }
}

impl From<RedHatBoyState<Falling>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<Falling>) -> Self {
        RedHatBoyStateMachine::Falling(state)
    }
}

impl From<RedHatBoyState<Idle>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<Idle>) -> Self {
        RedHatBoyStateMachine::Idle(state)
    }
}

impl From<RedHatBoyState<Jumping>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<Jumping>) -> Self {
        RedHatBoyStateMachine::Jumping(state)
    }
}

impl From<RedHatBoyState<KnockedOut>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<KnockedOut>) -> Self {
        RedHatBoyStateMachine::KnockedOut(state)
    }
}

impl From<RedHatBoyState<Running>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<Running>) -> Self {
        RedHatBoyStateMachine::Running(state)
    }
}

impl From<RedHatBoyState<Sliding>> for RedHatBoyStateMachine {
    fn from(state: RedHatBoyState<Sliding>) -> Self {
        RedHatBoyStateMachine::Sliding(state)
    }
}

impl From<SlidingEndState> for RedHatBoyStateMachine {
    fn from(end_state: SlidingEndState) -> Self {
        match end_state {
            SlidingEndState::Complete(running_state) => running_state.into(),
            SlidingEndState::Sliding(sliding_state) => sliding_state.into(),
        }
    }
}

struct Platform {
    sheet: Sheet,
    image: HtmlImageElement,
    position: Point,
}

impl Platform {
    fn bounding_boxes(&self) -> Vec<Rect> {
        const X_OFFSET: i16 = 60;
        const END_HEIGHT: i16 = 45;

        let destination_box = self.destination_box();

        let bounding_box_one = Rect::new_from_x_y(
            destination_box.x(),
            destination_box.y(),
            X_OFFSET,
            END_HEIGHT,
        );

        let bounding_box_two = Rect::new_from_x_y(
            destination_box.x() + X_OFFSET,
            destination_box.y(),
            destination_box.width - (X_OFFSET * 2),
            destination_box.height,
        );

        let bounding_box_three = Rect::new_from_x_y(
            destination_box.x() + destination_box.width - X_OFFSET,
            destination_box.y(),
            X_OFFSET,
            END_HEIGHT,
        );

        vec![bounding_box_one, bounding_box_two, bounding_box_three]
    }

    fn destination_box(&self) -> Rect {
        let platform = self
            .sheet
            .frames
            .get("13.png")
            .expect("13.png does not exist");

        Rect::new_from_x_y(
            self.position.x.into(),
            self.position.y.into(),
            (platform.frame.w * 3).into(),
            platform.frame.h.into(),
        )
    }

    fn draw(&self, renderer: &Renderer) {
        let platform = self
            .sheet
            .frames
            .get("13.png")
            .expect("13.png does not exist");

        renderer.draw_image(
            &self.image,
            &Rect::new_from_x_y(
                platform.frame.x.into(),
                platform.frame.y.into(),
                (platform.frame.w * 3).into(),
                platform.frame.h.into(),
            ),
            &self.destination_box(),
        )
    }

    fn new(sheet: Sheet, image: HtmlImageElement, position: Point) -> Self {
        Platform {
            sheet: sheet,
            image: image,
            position: position,
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct Sheet {
    pub frames: HashMap<String, Cell>,
}

#[derive(Deserialize, Clone)]
pub struct SheetRect {
    pub x: i16,
    pub y: i16,
    pub w: i16,
    pub h: i16,
}

pub struct Walk {
    boy: RedHatBoy,
    backgrounds: [Image; 2],
    obstacles: Vec<Box<dyn Obstacle>>,
}

impl Walk {
    fn velocity(&self) -> i16 {
        -self.boy.walking_speed()
    }
}

pub enum WalkTheDog {
    Loading,
    Loaded(Walk),
}

impl WalkTheDog {
    pub fn new() -> Self {
        WalkTheDog::Loading
    }
}

#[async_trait(?Send)]
impl Game for WalkTheDog {
    fn draw(&self, renderer: &Renderer) {
        renderer.clear(&Rect::new_from_x_y(0, 0, 600, 600));

        if let WalkTheDog::Loaded(walk) = self {
            walk.backgrounds.iter().for_each(|background| {
                background.draw(renderer);
            });
            walk.boy.draw(renderer);
            walk.obstacles.iter().for_each(|obstacle| {
                obstacle.draw(renderer);
            });
        }
    }

    async fn initialize(&self) -> Result<Box<dyn Game>> {
        match self {
            WalkTheDog::Loading => {
                let json = browser::fetch_json("rhb.json").await?;
                let background = engine::load_image("BG.png").await?;
                let stone = engine::load_image("Stone.png").await?;
                let platform_sheet = browser::fetch_json("tiles.json").await?;
                let platform = Platform::new(
                    platform_sheet.into_serde::<Sheet>()?,
                    engine::load_image("tiles.png").await?,
                    Point {
                        x: 400,
                        y: LOW_PLATFORM,
                    },
                );
                let rhb = RedHatBoy::new(
                    json.into_serde::<Sheet>()?,
                    engine::load_image("rhb.png").await?,
                );
                let background_width = background.width() as i16;

                Ok(Box::new(WalkTheDog::Loaded(Walk {
                    boy: rhb,
                    backgrounds: [
                        Image::new(background.clone(), Point { x: 0, y: 0 }),
                        Image::new(
                            background,
                            Point {
                                x: background_width,
                                y: 0,
                            },
                        ),
                    ],
                    obstacles: vec![
                        Box::new(Barrier::new(Image::new(stone, Point { x: 150, y: 546 }))),
                        Box::new(platform),
                    ],
                })))
            }

            WalkTheDog::Loaded(_) => Err(anyhow!("Error: Game is already initialized!")),
        }
    }

    fn update(&mut self, keystate: &KeyState) {
        if let WalkTheDog::Loaded(walk) = self {
            if keystate.is_pressed("ArrowDown") {
                walk.boy.slide();
            }

            if keystate.is_pressed("ArrowRight") {
                walk.boy.run_right();
            }

            if keystate.is_pressed("Space") {
                walk.boy.jump();
            }

            walk.boy.update();

            let velocity = walk.velocity();

            let [first_background, second_background] = &mut walk.backgrounds;
            first_background.move_horizontally(velocity);
            second_background.move_horizontally(velocity);

            if first_background.right() < 0 {
                first_background.set_x(second_background.right());
            }

            if second_background.right() < 0 {
                second_background.set_x(first_background.right());
            }

            walk.obstacles.retain(|obstacle| obstacle.right() > 0); // prune obstacles no longer on the screen
            walk.obstacles.iter_mut().for_each(|obstacle| {
                obstacle.move_horizontally(velocity);
                obstacle.check_intersection(&mut walk.boy);
            });
        }
    }
}

mod red_hat_boy_states {
    use super::HEIGHT;
    use crate::engine::Point;

    const FALLING_FRAMES: u8 = 29;
    const FALLING_FRAME_NAME: &str = "Dead";
    const FLOOR: i16 = 479;
    const GRAVITY: i16 = 1;
    const IDLE_FRAMES: u8 = 29;
    const IDLE_FRAME_NAME: &str = "Idle";
    const JUMPING_FRAME_NAME: &str = "Jump";
    const JUMPING_FRAMES: u8 = 35;
    const JUMPING_SPEED: i16 = -25;

    const PLAYER_HEIGHT: i16 = HEIGHT - FLOOR;
    const RUNNING_FRAMES: u8 = 23;
    const RUN_FRAME_NAME: &str = "Run";
    const RUNNING_SPEED: i16 = 4;
    const SLIDING_FRAMES: u8 = 14;
    const SLIDING_FRAME_NAME: &str = "Slide";
    const STARTING_POINT: i16 = -20;
    const TERMINAL_VELOCITY: i16 = 20;

    #[derive(Copy, Clone)]
    pub struct Falling;

    #[derive(Copy, Clone)]
    pub enum FallingEndState {
        Falling(RedHatBoyState<Falling>),
        KnockedOut(RedHatBoyState<KnockedOut>),
    }

    #[derive(Copy, Clone)]
    pub struct Idle;

    #[derive(Copy, Clone)]
    pub struct RedHatBoyContext {
        pub frame: u8,
        pub position: Point,
        pub velocity: Point,
    }

    impl RedHatBoyContext {
        fn reset_frame(mut self) -> Self {
            self.frame = 0;
            self
        }

        fn run_right(mut self) -> Self {
            self.velocity.x += RUNNING_SPEED;
            self
        }

        fn set_on(mut self, position: i16) -> Self {
            let position = position - PLAYER_HEIGHT;
            self.position.y = position;
            self
        }

        fn set_vertical_velocity(mut self, y: i16) -> Self {
            self.velocity.y = y;
            self
        }

        fn stop(mut self) -> Self {
            self.velocity.x = 0;
            self.velocity.y = 0;
            self
        }

        pub fn update(mut self, frame_count: u8) -> Self {
            if self.velocity.y < TERMINAL_VELOCITY {
                self.velocity.y += GRAVITY;
            }

            if self.frame < frame_count {
                self.frame += 1;
            } else {
                self.frame = 0;
            }

            self.position.y += self.velocity.y;

            if self.position.y > FLOOR {
                self.position.y = FLOOR;
            }

            self
        }
    }

    #[derive(Copy, Clone)]
    pub struct RedHatBoyState<S> {
        context: RedHatBoyContext,
        _state: S,
    }

    impl<S> RedHatBoyState<S> {
        pub fn context(&self) -> &RedHatBoyContext {
            &self.context
        }
    }

    impl RedHatBoyState<Falling> {
        pub fn frame_name(&self) -> &str {
            FALLING_FRAME_NAME
        }

        pub fn knock_out(self) -> RedHatBoyState<KnockedOut> {
            RedHatBoyState {
                context: self.context,
                _state: KnockedOut {},
            }
        }

        pub fn update(mut self) -> FallingEndState {
            self.context = self.context.update(FALLING_FRAMES);

            if self.context.frame >= FALLING_FRAMES {
                FallingEndState::KnockedOut(self.knock_out())
            } else {
                FallingEndState::Falling(self)
            }
        }
    }

    impl RedHatBoyState<Idle> {
        pub fn frame_name(&self) -> &str {
            IDLE_FRAME_NAME
        }

        pub fn new() -> Self {
            RedHatBoyState {
                context: RedHatBoyContext {
                    frame: 0,
                    position: Point {
                        x: STARTING_POINT,
                        y: FLOOR,
                    },
                    velocity: Point { x: 0, y: 0 },
                },
                _state: Idle {},
            }
        }

        pub fn run(self) -> RedHatBoyState<Running> {
            RedHatBoyState {
                context: self.context.reset_frame().run_right(),
                _state: Running {},
            }
        }

        pub fn update(mut self) -> Self {
            self.context = self.context.update(IDLE_FRAMES);
            self
        }
    }

    impl RedHatBoyState<Jumping> {
        pub fn frame_name(&self) -> &str {
            JUMPING_FRAME_NAME
        }

        pub fn knock_out(self) -> RedHatBoyState<Falling> {
            RedHatBoyState {
                context: self.context.reset_frame().stop(),
                _state: Falling {},
            }
        }

        pub fn land_on(self, position: i16) -> RedHatBoyState<Running> {
            RedHatBoyState {
                context: self.context.reset_frame().set_on(position as i16),
                _state: Running {},
            }
        }

        pub fn update(mut self) -> JumpingEndState {
            self.context = self.context.update(JUMPING_FRAMES);

            if self.context.position.y >= FLOOR {
                JumpingEndState::Complete(self.land_on(HEIGHT.into()))
            } else {
                JumpingEndState::Jumping(self)
            }
        }
    }

    impl RedHatBoyState<KnockedOut> {
        pub fn frame_name(&self) -> &str {
            FALLING_FRAME_NAME
        }
    }

    impl RedHatBoyState<Running> {
        pub fn frame_name(&self) -> &str {
            RUN_FRAME_NAME
        }

        pub fn jump(self) -> RedHatBoyState<Jumping> {
            RedHatBoyState {
                context: self
                    .context
                    .set_vertical_velocity(JUMPING_SPEED)
                    .reset_frame(),
                _state: Jumping {},
            }
        }

        pub fn knock_out(self) -> RedHatBoyState<Falling> {
            RedHatBoyState {
                context: self.context.reset_frame().stop(),
                _state: Falling {},
            }
        }

        pub fn land_on(self, position: i16) -> RedHatBoyState<Running> {
            RedHatBoyState {
                context: self.context.set_on(position as i16),
                _state: Running {},
            }
        }

        pub fn slide(self) -> RedHatBoyState<Sliding> {
            RedHatBoyState {
                context: self.context.reset_frame(),
                _state: Sliding {},
            }
        }

        pub fn update(mut self) -> Self {
            self.context = self.context.update(RUNNING_FRAMES);
            self
        }
    }

    impl RedHatBoyState<Sliding> {
        pub fn frame_name(&self) -> &str {
            SLIDING_FRAME_NAME
        }

        pub fn knock_out(self) -> RedHatBoyState<Falling> {
            RedHatBoyState {
                context: self.context.reset_frame().stop(),
                _state: Falling {},
            }
        }

        pub fn land_on(self, position: i16) -> RedHatBoyState<Sliding> {
            RedHatBoyState {
                context: self.context.set_on(position as i16),
                _state: Sliding {},
            }
        }

        pub fn stand(self) -> RedHatBoyState<Running> {
            RedHatBoyState {
                context: self.context.reset_frame(),
                _state: Running,
            }
        }

        pub fn update(mut self) -> SlidingEndState {
            self.context = self.context.update(SLIDING_FRAMES);

            if self.context.frame >= SLIDING_FRAMES {
                SlidingEndState::Complete(self.stand())
            } else {
                SlidingEndState::Sliding(self)
            }
        }
    }

    #[derive(Copy, Clone)]
    pub struct Jumping;

    #[derive(Copy, Clone)]
    pub enum JumpingEndState {
        Complete(RedHatBoyState<Running>),
        Jumping(RedHatBoyState<Jumping>),
    }

    #[derive(Copy, Clone)]
    pub struct KnockedOut;

    #[derive(Copy, Clone)]
    pub struct Running;

    #[derive(Copy, Clone)]
    pub struct Sliding;

    #[derive(Copy, Clone)]
    pub enum SlidingEndState {
        Complete(RedHatBoyState<Running>),
        Sliding(RedHatBoyState<Sliding>),
    }
}
