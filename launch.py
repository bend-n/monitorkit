for n in range(16):
    kitty ./.target/release/cpu @(n) &
kitty ./.target/release/temps &
kitty sudo ./.target/release/intel &
./.target/release/memory