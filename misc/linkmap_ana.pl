#!/bin/perl

#print("$ARGV[0]\n");

open(MAP, $ARGV[0]) or die("Error");

$section = "none";

while($x = <MAP>) {
    #print($x);

    if($x =~ /^[.]([^\s]+)/) {
        #print($x);
        $section = "none";
        if(($1 eq "vector_table") || ($1 eq "text") || ($1 eq "rodata") || ($1 eq "data") || ($1 eq "bss") || ($1 eq "uninit")) {
            $section = $1;
            #print("$section\n");
        }
    }
    elsif($section ne "none") {
        if($x =~ /\s0x([0-9a-f]+)\s+0x([0-9a-f]+).*\s([^\s]+)/) {
            $addr = $1;
            $size = hex($2);
            $file = $3;
            #print("$section $addr $size $file\n");
            $file =~ s/.*\\//;
            $file =~ s/\(.*\)$//;
            #print("$section $addr $size $file\n");
            $file =~ s/[-][0-9a-f]+[.].*//;
            #print("$section $addr $size $file\n");

            $total += $size;
            $subtotal{"$section, $file"} += $size;
        }
        if($x =~ /\s[*]fill[*]\s+0x([0-9a-f]+)\s+0x([0-9a-f]+)/) {
            $addr = $1;
            $size = hex($2);
            $file = "fill";
            #print("$section $addr $size $file\n");

            $total += $size;
            $subtotal{"$section, fill"} += $size;
        }
    }
}

print("$total\tTOTAL\n");

while(($k, $v) = each %subtotal) {
    print("$v\t$k\n");
}
