# W56 brief: Kshana 0.38 WS-E, machines and sensors

Read first: docs/briefs/_COMMON-0.38.md (rules), docs/plans/0.38-PLAN.md "Scope extension" WS-E, strategy memo
moves 4 and the ecosystem playbook item 3 and 6.

## Branch you own
- kshana: feat/machines-0.38 from origin/main.

## Items
E1 NovAtel ASCII/binary basics (BESTPOS, RANGE, RXSTATUS-class logs) and RTCM3 MSM4/MSM7 observation readers,
   read-only, implemented from publicly available reference guides/specification excerpts only (no paid RTCM text;
   if a field layout is only in the paid standard, stop and report). W54 owns UBX and SBF: don't touch those readers.
E2 ROS 2 node (integrations/ros2/): subscribes NavSatFix / ublox_ubx_msgs, runs receiver-trust live, publishes a trust
   topic. Python (rclpy) over the kshana Python binding; tests runnable without ROS via a mock; colcon package files.
E3 GNSS-SDR → receiver-trust adapter (its monitor and PVT outputs).
E4 RTKLIB .pos reader (engine) and a gnss_lib_py bridge (Python; optional dependency, licence checked).
E5 IMU log ingest (CSV with documented columns + one common open format) into hybrid/UKF scenarios and as a
   trust-monitor input.
E6 Exporters: Prometheus text endpoint for trust-telemetry (if not already present, extend deploy/grafana) and a
   Home Assistant / NTP-appliance friendly JSON/MQTT exporter. Read-only to devices.

## Done when
Every new reader/command on every surface (or N/A with reason); integration docs under integrations/ and
docs/SURFACES.md; fixtures self-written or licence-recorded; gates green; report per common rules.
