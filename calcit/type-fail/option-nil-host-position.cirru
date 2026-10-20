
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-option-nil-host-position
  :entries $ {} $ :default
    {}
      :description "|Option-to-nil advisories are exempt only where the nil is a direct JS host argument."
      :init-fn 'type-fail-option-nil-host-position.main/main!
      :mode :native
      :reload-fn 'type-fail-option-nil-host-position.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-option-nil-host-position.main
    %{} 'FileEntry
      :defs $ {}
        'bound-in-ffi $ %{} 'CodeEntry
          :doc "|A local binding is not a host position even inside a `:js-ffi` definition."
          :code $ quote $ defn bound-in-ffi (m)
            let
                v $ .unwrap-or (get m :a) nil
              js/console.log v
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :features $ #{} :js-ffi
        'business-in-ffi $ %{} 'CodeEntry
          :doc "|The nil is tested by business code before reaching the host."
          :code $ quote $ defn business-in-ffi (m)
            js/console.log $ if
              nil? $ .unwrap-or (get m :a) nil
              , 1 2
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :features $ #{} :js-ffi
        'main! $ %{} 'CodeEntry (:doc "|Calls every adapter.")
          :code $ quote $ defn main! ()
            let
                m $ {} $ :a 1
              to-host m
              to-host-match m
              to-native-method m
              business-in-ffi m
              bound-in-ffi m
              , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'to-host $ %{} 'CodeEntry (:doc "|Passes the absence straight to a JS global.")
          :code $ quote $ defn to-host (m)
            js/console.log $ .unwrap-or (get m :a) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :features $ #{} :js-ffi
        'to-host-match $ %{} 'CodeEntry
          :doc "|A `(:none) nil` branch that is the host argument itself."
          :code $ quote $ defn to-host-match (m)
            js/console.log $ match (get m :a)
              (:some v) v
              (:none) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :features $ #{} :js-ffi
        'to-native-method $ %{} 'CodeEntry
          :doc "|A native method call argument is a host position."
          :code $ quote $ defn to-native-method (m)
            .!log (unsafe-coerce js/console 'Dynamic)
              .unwrap-or (get m :a) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :features $ #{} :js-ffi
      :ns $ %{} 'NsEntry (:doc "|Option-to-nil host position fixture.")
        :code $ quote $ ns type-fail-option-nil-host-position.main
