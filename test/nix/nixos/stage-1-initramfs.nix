{ busybox, hostPlatform, stdenv, makeInitrd, linux_vdso, compressed, }:
let
  image = makeInitrd {
    compressor = if compressed then "gzip" else "cat";
    contents = [
      {
        object = ./stage-1-init.sh;
        symlink = "/init";
      }
      {
        object = "${busybox}/bin";
        symlink = "/bin";
      }
      {
        object = if hostPlatform.isx86_64 then
          "${linux_vdso}/vdso64.so"
        else if hostPlatform.isRiscV64 then
          "${linux_vdso}/riscv64-vdso.so"
        else
          "";
        symlink = "/lib/x86_64-linux-gnu/vdso64.so";
      }
    ];
  };
in stdenv.mkDerivation {
  name = "stage-1-initramfs";
  buildCommand = ''
    ln -s ${image}/initrd $out
  '';
}
