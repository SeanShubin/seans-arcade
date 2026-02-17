// Function Wrapping — How frameworks accept plain functions
//
// In Bevy:  app.add_systems(Update, my_system)
// In Axum:  router.route("/", get(my_handler))
//
// Both accept a bare function where a complex trait is expected.
// The compiler automatically wraps the function using trait impls.
// This example builds the mechanism step by step, without Bevy.

fn main() {
    functions_are_values();
    blanket_impl();
    into_conversion();
    parameter_extraction();
}

// ── FUNCTIONS ARE VALUES ────────────────────────────────────────────
// Every function has a unique type that implements Fn/FnMut/FnOnce.
// You can pass functions wherever a callable trait is expected.

fn say_hello() {
    println!("[values] hello!");
}

fn functions_are_values() {
    run_it(say_hello);
    run_it(|| println!("[values] closures work too"));
}

fn run_it(f: impl Fn()) {
    // impl Fn() means: "any type that can be called with no arguments"
    // Both function pointers and closures qualify.
    f();
}

// ── BLANKET IMPL ON Fn ──────────────────────────────────────────────
// Define your own trait, then implement it for all callable types at once.
// This one blanket impl makes every fn() automatically implement your trait.

trait Callable {
    fn call(&self);
}

// "Anything that implements Fn() also implements Callable."
// This single impl covers every function and closure with the right signature.
impl<F: Fn()> Callable for F {
    fn call(&self) {
        self();
    }
}

fn blanket_impl() {
    // say_hello is fn(), which implements Fn(), which implements Callable.
    // The compiler found the impl through two hops: fn → Fn() → Callable.
    accept_callable(say_hello);
    accept_callable(|| println!("[blanket] closures too"));
}

fn accept_callable(c: impl Callable) {
    c.call();
}

// ── THE INTO CONVERSION PATTERN ─────────────────────────────────────
// Instead of calling the function directly, convert it into a wrapper struct.
// The wrapper can carry metadata alongside the function.
// This is what Bevy does: IntoSystem wraps your function in a FunctionSystem struct.

struct Job {
    name: &'static str,
    func: Box<dyn Fn()>,
}

impl Job {
    fn run(&self) {
        println!("[into] running '{}'", self.name);
        (self.func)();
    }
}

trait IntoJob {
    fn into_job(self) -> Job;
}

// Blanket impl: any Fn() can be converted into a Job.
// std::any::type_name gives us the function's name for free.
impl<F: Fn() + 'static> IntoJob for F {
    fn into_job(self) -> Job {
        Job {
            name: std::any::type_name::<F>(),
            func: Box::new(self),
        }
    }
}

fn into_conversion() {
    // Explicit conversion:
    let job = say_hello.into_job();
    job.run();

    // Framework does the conversion for you:
    schedule(say_hello);
}

fn schedule(handler: impl IntoJob) {
    // The caller passes a bare function.
    // This function calls .into_job() to get the wrapper.
    // The caller never sees the wrapper type.
    let job = handler.into_job();
    job.run();
}

// ── PARAMETER EXTRACTION ──────────────────────────────────────────────
// The full pattern. Functions declare what they need as parameters.
// The framework extracts each parameter from shared state and calls the function.
//
// This is the mechanism behind Bevy's systems:
//   fn my_system(time: Res<Time>, query: Query<&Transform>) { ... }
//   Bevy sees two parameters, extracts both from the World, calls my_system(time, query).
//
// Below is a minimal working version.

// Shared state the framework manages — like Bevy's World
struct Context {
    frame: i32,
    player_name: String,
}

// "I can be extracted from a Context" — like Bevy's SystemParam
trait Extract {
    fn extract(ctx: &Context) -> Self;
}

// Two extractable types. Each knows how to pull its data from Context.

struct Frame(i32);

impl Extract for Frame {
    fn extract(ctx: &Context) -> Self {
        Frame(ctx.frame)
    }
}

struct PlayerName(String);

impl Extract for PlayerName {
    fn extract(ctx: &Context) -> Self {
        PlayerName(ctx.player_name.clone())
    }
}

// The wrapper — holds a closure that knows how to extract params and call the function
struct Runner {
    run_fn: Box<dyn Fn(&Context)>,
}

// The conversion trait. The Marker generic is important — it distinguishes:
//   impl IntoRunner<()>          for Fn()
//   impl IntoRunner<(P0,)>      for Fn(P0)
//   impl IntoRunner<(P0, P1)>   for Fn(P0, P1)
// Without Marker, Rust would see these as conflicting impls on the same trait.
trait IntoRunner<Marker> {
    fn into_runner(self) -> Runner;
}

// Zero parameters — nothing to extract, just call it
impl<F: Fn() + 'static> IntoRunner<()> for F {
    fn into_runner(self) -> Runner {
        Runner {
            run_fn: Box::new(move |_ctx| {
                self();
            }),
        }
    }
}

// One parameter — extract P0 from Context, pass it to the function
impl<F, P0> IntoRunner<(P0,)> for F
where
    F: Fn(P0) + 'static,
    P0: Extract + 'static,
{
    fn into_runner(self) -> Runner {
        Runner {
            run_fn: Box::new(move |ctx| {
                let p0 = P0::extract(ctx);
                self(p0);
            }),
        }
    }
}

// Two parameters — extract both, pass both
impl<F, P0, P1> IntoRunner<(P0, P1)> for F
where
    F: Fn(P0, P1) + 'static,
    P0: Extract + 'static,
    P1: Extract + 'static,
{
    fn into_runner(self) -> Runner {
        Runner {
            run_fn: Box::new(move |ctx| {
                let p0 = P0::extract(ctx);
                let p1 = P1::extract(ctx);
                self(p0, p1);
            }),
        }
    }
}

// Bevy uses a macro to generate these impls for up to 16 parameters.
// The pattern is identical each time — just more extract() calls.

// A mini scheduler that collects and runs handlers
struct Scheduler {
    runners: Vec<Runner>,
}

impl Scheduler {
    fn new() -> Self {
        Scheduler { runners: vec![] }
    }

    // M is inferred from the function's signature.
    // The compiler picks the matching IntoRunner impl automatically.
    fn add<M>(&mut self, handler: impl IntoRunner<M>) {
        self.runners.push(handler.into_runner());
    }

    fn run_all(&self, ctx: &Context) {
        for runner in &self.runners {
            (runner.run_fn)(ctx);
        }
    }
}

// Three plain functions with different signatures:

fn tick() {
    println!("[extract] tick (no params)");
}

fn print_frame(frame: Frame) {
    println!("[extract] frame: {}", frame.0);
}

fn greet_player(name: PlayerName, frame: Frame) {
    println!("[extract] hello {} on frame {}", name.0, frame.0);
}

fn parameter_extraction() {
    let mut scheduler = Scheduler::new();

    // All three register through .add() despite different signatures.
    // The compiler picks the right IntoRunner impl for each:
    //   tick         → Fn()             → IntoRunner<()>
    //   print_frame  → Fn(Frame)        → IntoRunner<(Frame,)>
    //   greet_player → Fn(PlayerName, Frame) → IntoRunner<(PlayerName, Frame)>
    scheduler.add(tick);
    scheduler.add(print_frame);
    scheduler.add(greet_player);

    let ctx = Context {
        frame: 42,
        player_name: "Sean".into(),
    };

    // The scheduler calls each function with its extracted parameters.
    // tick()               gets nothing
    // print_frame(frame)   gets Frame(42)
    // greet_player(n, f)   gets PlayerName("Sean"), Frame(42)
    scheduler.run_all(&ctx);
}
