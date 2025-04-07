for n in range(16):
    kitty ./.target/release/cpu @(n) &
kitty ./.target/release/temps &
kitty ./.target/release/disk nvme r &
kitty ./.target/release/ping google.com &
# kitty sudo ./.target/release/intel &
./.target/release/memory
