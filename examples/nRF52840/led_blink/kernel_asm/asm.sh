ASPATH='/c/Program Files (x86)/GNU Tools ARM Embedded/8 2019-q3-update/bin/arm-none-eabi-as'

"$ASPATH" -march=armv6s-m     --defsym V6=1 kernel_asm.s -g -o thumbv6m-none-eabi_kernel_asm.o
"$ASPATH" -march=armv7-m      --defsym V7=1 kernel_asm.s -g -o thumbv7m-none-eabi_kernel_asm.o
"$ASPATH" -march=armv7e-m     --defsym V7=1 kernel_asm.s -g -o thumbv7em-none-eabi_kernel_asm.o
"$ASPATH" -march=armv7e-m     --defsym V7=1 kernel_asm.s -g -o thumbv7em-none-eabihf_kernel_asm.o
"$ASPATH" -march=armv8-m.base --defsym V8=1 kernel_asm.s -g -o thumbv8m.base-none-eabi_kernel_asm.o
"$ASPATH" -march=armv8-m.main --defsym V8=1 kernel_asm.s -g -o thumbv8m.main-none-eabi_kernel_asm.o
