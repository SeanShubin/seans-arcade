# Window Modes

How game windows interact with the OS display, and what each mode means for performance, usability, and alt-tabbing.

## The Three Standard Modes

### Windowed

A regular OS window with title bar, borders, and resize handles. Can be moved, resized, and minimized like any application. Alt-tabbing is instant because the OS compositor still manages the desktop. May have slightly lower performance due to desktop composition overhead.

### Fullscreen (Exclusive)

The application takes exclusive control of the display output, bypassing the OS compositor and rendering directly to the screen. Can change the monitor's actual resolution and refresh rate. Offers the best potential performance, but alt-tabbing is slow because the app must release and reacquire the display. Other windows cannot draw on top.

### Borderless Windowed (Borderless Fullscreen)

A regular window sized exactly to the monitor with no title bar or borders. Looks like fullscreen but is actually a maximized window. The OS compositor still runs, so alt-tabbing is instant and overlays/notifications work normally. Has slightly more latency than exclusive fullscreen (an extra frame of compositor delay). Cannot change the monitor's resolution — uses whatever the desktop is set to.

## Other Modes

**Windowed Borderless (not fullscreen-sized)** — A borderless window at a custom size, floating on the desktop. Useful for streaming setups or multi-monitor workflows.

**Fullscreen Optimized / Flip Model (Windows 10+)** — Microsoft blurred the line between borderless and exclusive fullscreen. Modern Windows automatically optimizes borderless fullscreen windows to get near-exclusive-fullscreen performance. Many "fullscreen" games on Windows 10/11 actually use this mode internally.

**Multi-monitor / Spanning** — Some applications stretch across multiple monitors in fullscreen.

## Comparison

| Feature               | Windowed           | Borderless         | Exclusive Fullscreen |
|-----------------------|--------------------|--------------------| ---------------------|
| Alt-tab speed         | Instant            | Instant            | Slow                 |
| Custom resolution     | No (uses desktop)  | No (uses desktop)  | Yes                  |
| Performance           | Good               | Good               | Best (marginal)      |
| Overlays/popups       | Work normally      | Work normally      | Often blocked        |
| Screen tearing control| Compositor handles | Compositor handles | App/driver handles   |

## Bevy API

Bevy exposes these via `WindowMode`:

| Bevy variant                          | Mode                                               |
|---------------------------------------|----------------------------------------------------|
| `WindowMode::Windowed`                | Standard windowed                                  |
| `WindowMode::BorderlessFullscreen(m)` | Borderless window sized to monitor                 |
| `WindowMode::Fullscreen(m)`           | Exclusive fullscreen (can change resolution)       |
| `WindowMode::SizedFullscreen(m)`      | Exclusive fullscreen at desktop resolution          |

The `m` parameter selects which monitor to use.

## Practical Note

On modern Windows (10+), the performance gap between borderless and exclusive fullscreen is negligible due to the flip model compositor optimization. Most players prefer borderless fullscreen for the instant alt-tabbing and overlay support.
