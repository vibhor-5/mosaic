class Mosaic < Formula
  desc "Blazing fast, natively integrated macOS Tiling Window Manager"
  homepage "https://github.com/vibhor-5/mosaic"
  url "https://github.com/vibhor-5/mosaic/releases/download/v0.1.0/mosaic-macos-universal.tar.gz"
  sha256 "REPLACE_WITH_SHA256_OF_TARBALL"
  version "0.1.0"
  depends_on :macos

  def install
    bin.install "mosaic"
    bin.install "mosaic-msg"
    
    # Install the Scripting Addition payload to a shared lib folder
    lib.install "payload.dylib"
    
    # Install helper scripts
    pkgshare.install "install-sa.sh"
    pkgshare.install "install-service.sh"
    
    # Install example config
    pkgshare.install "mosaic.toml"
  end

  def caveats
    <<~EOS
      To enable instant Space switching, you must inject the Scripting Addition into Dock.app:
        sudo #{pkgshare}/install-sa.sh

      To start the mosaic daemon automatically on login:
        brew services start mosaic

      Configuration file should be placed at:
        ~/.config/mosaic/mosaic.toml
        
      (An example config can be found at #{pkgshare}/mosaic.toml)
    EOS
  end

  service do
    run [opt_bin/"mosaic"]
    keep_alive true
    log_path var/"log/mosaic.log"
    error_log_path var/"log/mosaic.err.log"
  end
end
