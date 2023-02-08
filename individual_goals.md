# Individual goals document

## Goal: Continuos integration/deployment through Github Actions

Have a Github action compile and make binary releases available for download. This will make it very easy
to deploy to new units since Github will host a binary for download. This could be further automated by
writing a convenience script to update the binary.

## Goal: Investigate poor TCP performance

We would like to consistently have 100 Kbps or higher throughput on a TCP link. This will be
evaluated using `iperf3`.

## Goal: VPN mode

Be able to route all traffic through our network and to another device. This is basically emulating a LTE network
by having IP traffic pass through a base station. We will ping and iperf public servers to see how much
the performance is affected.

## Goal: More friendly cli

The CLI should have multiple flags and explain how they are used. This will greatly improve usability.

## Goal: Install systemd service through cli

Allow the program to install itself into the system with a flag. This will make deployments easier.

## Goal: Increase overall throughput

All optimization have been done by limited testing and guesswork. We would like to profile and instrument
the code to see where and if there are any further optimizations to be done. This will be evaluated by
UDP and TCP throughput.

## Goal: Test max distance between units

We would like to test how far units can be from eachother and still have a acceptable ping (<30ms). This
will be plotted in a graph to show response time vs distance.
