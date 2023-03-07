# Individual goals document

## Goal: Continuos integration/deployment through Github Actions

Have a Github action compile and make binary releases available for download. This will make it very easy
to deploy to new units since Github will host a binary for download. This could be further automated by
writing a convenience script to update the binary.

### Update:

This was achieved with Github actions as described above which eased development since other members of the
group did not need to setup a build environment for the armv7 build target.

## Goal: Investigate poor TCP performance

We would like to consistently have 100 Kbps or higher throughput on a TCP link. This will be
evaluated using `iperf3`.

### Update:

TCP performance was increased by making sure less packets are dropped by increasing the retry delay and count.
Some other performance tuning was done but this had the most effect and in turn made UDP more stable.

## Goal: VPN mode

Be able to route all traffic through our network and to another device. This is basically emulating a LTE network
by having IP traffic pass through a base station. We will ping and iperf public servers to see how much
the performance is affected.

### Update:

The implemented tunnel mode does not change the devices routing table (but it could be done) and requires
static routing within the longge network. The bandwidth is limited to ~200Kbit/s and around 5-15ms extra latency.

## Goal: More friendly cli

The CLI should have multiple flags and explain how they are used. This will greatly improve usability.

### Update:

This was implemented using [clap](https://crates.io/crates/clap) and explains what options the program can receive.

## Goal: Install systemd service through cli

Allow the program to install itself into the system with a flag. This will make deployments easier.

### Update:

This goal was not worked on in favour of the other goals.

## Goal: Increase overall throughput

All optimization have been done by limited testing and guesswork. We would like to profile and instrument
the code to see where and if there are any further optimizations to be done. This will be evaluated by
UDP and TCP throughput.

### Update:

This is closely linked to the TCP performance goal and we hade some success with increasing overall performance.
The latency is somewhat slower after adding addressing.

## Goal: Test max distance between units

We would like to test how far units can be from eachother and still have a acceptable ping (<30ms). This
will be plotted in a graph to show response time vs distance.

### Update:

This goal was not worked on i favour of the other goals.
