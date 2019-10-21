AS='/c/Program Files (x86)/GNU Tools ARM Embedded/8 2019-q3-update/bin/arm-none-eabi-as'
AR='/c/Program Files (x86)/GNU Tools ARM Embedded/8 2019-q3-update/bin/arm-none-eabi-ar'

rm -f *.a

"$AS" -march=armv6s-m     --defsym V6=1 minimult_asm.s -g -o thumbv6m-none-eabi_minimult_asm.o
"$AR" crs thumbv6m-none-eabi_minimult_asm.a thumbv6m-none-eabi_minimult_asm.o

"$AS" -march=armv7-m      --defsym V7=1 minimult_asm.s -g -o thumbv7m-none-eabi_minimult_asm.o
"$AR" crs thumbv7m-none-eabi_minimult_asm.a thumbv7m-none-eabi_minimult_asm.o

"$AS" -march=armv7e-m     --defsym V7=1 minimult_asm.s -g -o thumbv7em-none-eabi_minimult_asm.o
"$AR" crs thumbv7em-none-eabi_minimult_asm.a thumbv7em-none-eabi_minimult_asm.o

"$AS" -march=armv7e-m     --defsym V7=1 minimult_asm.s -g -o thumbv7em-none-eabihf_minimult_asm.o
"$AR" crs thumbv7em-none-eabihf_minimult_asm.a thumbv7em-none-eabihf_minimult_asm.o

"$AS" -march=armv8-m.base --defsym V8=1 minimult_asm.s -g -o thumbv8m.base-none-eabi_minimult_asm.o
"$AR" crs thumbv8m.base-none-eabi_minimult_asm.a thumbv8m.base-none-eabi_minimult_asm.o

"$AS" -march=armv8-m.main --defsym V8=1 minimult_asm.s -g -o thumbv8m.main-none-eabi_minimult_asm.o
"$AR" crs thumbv8m.main-none-eabi_minimult_asm.a thumbv8m.main-none-eabi_minimult_asm.o

"$AS" -march=armv8-m.main --defsym V8=1 minimult_asm.s -g -o thumbv8m.main-none-eabihf_minimult_asm.o
"$AR" crs thumbv8m.main-none-eabihf_minimult_asm.a thumbv8m.main-none-eabihf_minimult_asm.o

rm -f *.o
