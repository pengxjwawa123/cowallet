Pod::Spec.new do |s|
  s.name         = 'ffi_mobile'
  s.version      = '1.0.0'
  s.summary      = 'CoWallet Rust FFI library'
  s.homepage     = 'https://github.com/nicejuice-cc/cowallet'
  s.license      = { :type => 'Proprietary' }
  s.author       = 'CoWallet'
  s.source       = { :path => '.' }
  s.platform     = :ios, '12.0'
  s.vendored_frameworks = 'ffi_mobile.xcframework'
end
