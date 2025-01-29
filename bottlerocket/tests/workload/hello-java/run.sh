#! /usr/bin/env bash

set -e
set -o pipefail

# Collect the test output where sonobuoy expects plugins to place them
results_dir="${RESULTS_DIR:-/tmp/results}"
results_tar="results.tar.gz"
mkdir -p "${results_dir}"

testDone() {
    echo "${results_dir}/${results_tar}" >"${results_dir}/done"
}

# Make sure to always output done file in expected place and format
trap testDone EXIT

hello_script="$(mktemp --suffix='.java')"

cat >"${hello_script}" <<EOF
public class Main {
  public static void main(String[] args) {
    System.out.println("hello, java");
  }
}
EOF

java "${hello_script}" |
    tee "${results_dir}/hello.log"

# Collect the results
tar czf "${results_tar}" -C "${results_dir}" .
mv "${results_tar}" "${results_dir}/"
