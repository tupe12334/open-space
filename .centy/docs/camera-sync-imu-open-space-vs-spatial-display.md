---
createdAt: 2026-03-07T14:06:29.868089+00:00
updatedAt: 2026-03-07T14:06:29.868089+00:00
---

# Camera Sync & IMU: open-space vs spatial-display

# Camera Sync & IMU: open-space vs spatial-display

Comparison of head tracking and screen capture between our project (open-space) and the reference project (spatial-display). Documents why our camera sync is broken and what to fix.

## IMU Filter

- **spatial-display (reference):** Uses DCMIMU — purpose-built for 6-DOF head tracking. Outputs Euler angles (yaw/pitch/roll). No calibration needed. Uses IMU hardware timestamps for delta time.
- **open-space (ours):** Uses Madgwick AHRS — general-purpose filter requiring tuning. Outputs quaternion. Requires 3s warmup + recalibration after 100 samples. Uses wall-clock `Instant::now()` for delta time.

## Root Cause: Quaternion Coordinate Mapping

This is the primary bug.

spatial-display explicitly remaps IMU axes to Bevy's coordinate system:

```rust
// spatial-display/src/hmd.rs:104-109
Quat::from_euler(EulerRot::YXZ, dcm.yaw, -dcm.roll, dcm.pitch)
```

- Uses YXZ rotation order
- Inverts roll (-dcm.roll)
- Maps each IMU axis to the correct Bevy axis

open-space passes the Madgwick quaternion directly with no remapping:

```rust
// open-space/src/hmd.rs:121-126
Quat::from_xyzw(relative.i, relative.j, relative.k, relative.w)
```

The IMU's coordinate frame does not match Bevy's. Without remapping, head movements map to wrong rotations (e.g., tilting left might roll instead of yaw, pitch/yaw could be swapped or inverted).

## Secondary Issue: Filter Recalibration Resets State

After 100 samples, open-space replaces the entire Madgwick filter:

```rust
// open-space/src/hmd.rs:86
ahrs = Madgwick::new(avg_dt, 0.1);
```

This discards all accumulated orientation state. The filter must reconverge from scratch, producing unstable orientation output during that period.

## Secondary Issue: Schedule Timing

- **spatial-display:** Uses `FixedPreUpdate` — camera updates before other systems, minimizing latency.
- **open-space:** Uses `FixedUpdate` — camera updates alongside other systems, adds up to one fixed-step of latency (~2ms at 500Hz).

## Screen Capture

The screen capture implementations are nearly identical. Both use ScreenCaptureKit with BGRA pixel format at 60 FPS, BGRA-to-RGBA CPU conversion in the delegate callback, tokio MPSC channels (capacity 60) for frame delivery, and per-display serial dispatch queues.

One difference: open-space drains all pending frames and keeps the latest (while let Ok(...)), while spatial-display takes only one per tick (try_recv() once). Our approach is actually better here.

## Fix Options

### Option A: Switch to DCMIMU

Replace Madgwick with DCMIMU. This is the simplest path since it is proven to work for this exact use case and outputs Euler angles that are easy to remap.

### Option B: Keep Madgwick, Add Axis Remapping

After computing the relative quaternion, convert to Euler angles, remap/invert axes to match Bevy's coordinate system, then convert back. Also fix the recalibration issue by not recreating the filter.

### Other Fixes

- Change FixedUpdate to FixedPreUpdate for lower latency.
- Use IMU hardware timestamps instead of wall-clock time for more accurate delta-time.
- Remove or defer the filter recalibration so it does not reset accumulated state.
