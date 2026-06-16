class Mosaic < Formula
  desc "Blazing fast, natively integrated macOS Tiling Window Manager"
  homepage "https://github.com/vibhor-5/mosaic"
  url "https://github.com/vibhor-5/mosaic/releases/download/v0.1.3/mosaic-macos-universal.tar.gz"
  sha256 "c5c884e6bbb7c2f0f93f7d446b180f5ba4f39083317c22d98a29cb6bcd3e8cfe"
  version "0.1.3"
  depends_on :macos

  def install
    bin.install "mosaic"
    bin.install "mosaic-msg"
    
    # Install the Scripting Addition payload to a shared lib folder
    lib.install "payload.dylib"
    
    # Install helper script as an executable in the path
    bin.install "install-sa.sh" => "mosaic-inject"
    pkgshare.install "install-service.sh"
    
    # Install example config
    pkgshare.install "mosaic.toml"
  end

  def caveats
    <<~EOS
      To enable instant Space switching, you must inject the Scripting Addition into Dock.app:
        sudo mosaic-inject

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
