
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
        'HostConsole $ %{} 'CodeEntry (:doc "|An external-object console.")
          :code $ quote $ deftrait HostConsole
            .log $ :: 'Fn $ {}
              :args $ [] 'type-fail-option-nil-host-position.main/HostConsole 'Dynamic
              :return 'Unit
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object)
          :schema $ :: 'Trait
        'HostObject $ %{} 'CodeEntry (:doc "|An external-object with a writable field.")
          :code $ quote $ deftrait HostObject (:a 'Dynamic)
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object)
            :writable $ #{} :a
          :schema $ :: 'Trait
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
        'local-set-in-ffi $ %{} 'CodeEntry
          :doc "|`set!` on a lexical local rebinds a Calcit value, not a host field."
          :code $ quote $ defn local-set-in-ffi (m)
            let
                v $ unsafe-coerce 1 'Dynamic
              set! v $ .unwrap-or (get m :a) nil
              , v
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
              local-set-in-ffi m
              to-host-field m
              to-external-method m
              to-external-postfix m
              to-native-postfix m
              to-nullish-param $ {} (:a 1)
              to-plain-param $ {} (:a 1)
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
        'take-host-value $ %{} 'CodeEntry
          :doc "|A Calcit function whose parameter declares a JS nullish contract."
          :code $ quote $ defn take-host-value (x) &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] $ :: 'JsNullish 'Number
        'take-plain-value $ %{} 'CodeEntry
          :doc "|A Calcit function with an ordinary parameter."
          :code $ quote $ defn take-plain-value (x) &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] 'Dynamic
        'to-nullish-param $ %{} 'CodeEntry
          :doc "|An argument for a `JsNullish` parameter accepts nil as declared."
          :code $ quote $ defn to-nullish-param (m)
            take-host-value $ match (get m :a)
              (:some v) v
              (:none) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] $ :: 'Map 'Tag 'Number
        'to-plain-param $ %{} 'CodeEntry
          :doc "|An ordinary parameter is not a host position."
          :code $ quote $ defn to-plain-param (m)
            take-plain-value $ match (get m :a)
              (:some v) v
              (:none) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ [] $ :: 'Map 'Tag 'Number
        'to-external-method $ %{} 'CodeEntry
          :doc "|An external-object method argument is a host position."
          :code $ quote $ defn to-external-method (m)
            let
                c $ unsafe-coerce js/console type-fail-option-nil-host-position.main/HostConsole
              .log c $ .unwrap-or (get m :a) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :features $ #{} :js-ffi
        'to-external-postfix $ %{} 'CodeEntry
          :doc "|The postfix external-object method form is a host position too."
          :code $ quote $ defn to-external-postfix (m)
            let
                c $ unsafe-coerce js/console type-fail-option-nil-host-position.main/HostConsole
              c .log $ .unwrap-or (get m :a) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :features $ #{} :js-ffi
        'to-host-field $ %{} 'CodeEntry
          :doc "|A `set!` on a host property writes to JS."
          :code $ quote $ defn to-host-field (m)
            let
                o $ unsafe-coerce (js-object) type-fail-option-nil-host-position.main/HostObject
              set! (.-a o) $ .unwrap-or (get m :a) nil
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] $ :: 'Map 'Tag 'Dynamic
            :features $ #{} :js-ffi
        'to-native-postfix $ %{} 'CodeEntry
          :doc "|The postfix native method form is a host position too."
          :code $ quote $ defn to-native-postfix (m)
            let
                c $ unsafe-coerce js/console type-fail-option-nil-host-position.main/HostConsole
              c .!log $ .unwrap-or (get m :a) nil
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
