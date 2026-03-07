# Drift Correction

## The Problem

IMU gyroscopes accumulate small integration errors over time, causing the estimated orientation to slowly "drift" away from the true heading. This is the main tracking accuracy issue in open-space.

## How Drift Happens

- **Gyroscope bias**: tiny constant offset in gyro readings integrates into a growing angle error.
- **Noise accumulation**: random sensor noise adds up each integration step.
- **Temperature changes**: bias shifts as the IMU warms up.

## Axes Affected Differently

| Axis | Correctable via accelerometer? | Why |
|------|-------------------------------|-----|
| Pitch | Yes | Gravity provides a reference |
| Roll | Yes | Gravity provides a reference |
| Yaw | No | Gravity is the same at any heading |

Yaw drift is the hardest to fix because the accelerometer cannot distinguish between yaw angles.

## Current Approach (`drift_correction.rs`)

When the user is looking roughly forward (within ~5° of the calibration point), the calibration offsets are slowly nudged toward the current IMU reading at 10%/sec. This only activates near center and does nothing for yaw drift during active head movement.

## Common Fix Strategies

### Complementary Filter

Blend gyroscope (accurate short-term) with accelerometer (no drift on pitch/roll):

```
orientation = alpha * gyro_integrated + (1 - alpha) * accel_derived
```

The `dcmimu` crate already does this internally for pitch and roll.

### Madgwick / Mahony Filter

Quaternion-based sensor fusion algorithms that continuously correct gyro integration using accelerometer and magnetometer data. Benefits over DCM:

- No gimbal lock at extreme tilt angles.
- Built-in gyro bias estimation and correction.
- Lower computational cost (Madgwick).

The [xioTechnologies/Fusion](https://github.com/xioTechnologies/Fusion) library is a well-maintained implementation.

### Magnetometer (Compass)

Provides an absolute yaw reference. Downsides: sensitive to nearby magnets, motors, and metal.

### Visual Anchoring

Use a camera to detect fixed reference points and correct yaw. This is how inside-out tracking headsets (Quest, Vision Pro) solve yaw drift. The existing webcam face-tracking system could serve as a yaw reference.

### Rest Detection

When the IMU detects no motion, zero out the gyro bias to prevent further accumulation.

## Known Issues

- **Gimbal lock at ~90° tilt**: the DCM / Euler angle representation breaks down. Switching to a quaternion-based filter (Madgwick) would fix this.
- **Yaw drift during use**: no absolute yaw reference exists yet. Visual anchoring via the webcam is the most promising path.

## References

- [Complementary Filtering Guide](https://guidenav.com/blog/how-gyroscopes-and-accelerometers-shape-imu-performance/)
- [Madgwick Filter Explained](https://qsense-motion.com/qsense-imu-motion-sensor/madgwick-filter-sensor-fusion/)
- [xioTechnologies/Fusion](https://github.com/xioTechnologies/Fusion)
- [Head Tracker — IMU Calibration and Drift](https://headtracker.gitbook.io/head-tracker/getting-started/imu-calibration-and-drift)
- [navX-MXP — Yaw Drift](https://pdocs.kauailabs.com/navx-mxp/guidance/yaw-drift/)
