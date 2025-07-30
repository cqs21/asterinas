{ stdenv, writeClosure, e2fsprogs, fakeroot, toplevel, ... }:
stdenv.mkDerivation {
  name = "stage-2-rootfs";
  nativeBuildInputs = [ e2fsprogs fakeroot ];
  buildCommand = ''
    tmp_dir=$(mktemp -d)
    rootfs=$tmp_dir/rootfs
    img_file=$tmp_dir/rootfs.img

    mkdir -p $rootfs/{dev,proc,sys}
    cp -r ${toplevel}/sw/* $rootfs/

    mkdir -p $rootfs/nix/store
    pkg_path=${toplevel.outPath}
    while IFS= read -r dep_path; do
      if [[ "$pkg_path" == *"$dep_path"* ]]; then
        continue
      fi
      cp -r $dep_path $rootfs/nix/store/
    done < ${writeClosure toplevel}

    dd if=/dev/zero of=$img_file bs=1M count=2048
    fakeroot mke2fs -t ext2 -d $rootfs $img_file

    mv $img_file $out
  '';
}
