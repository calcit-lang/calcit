
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-host-value-equality-strict
  :entries $ {} $ :default
    {}
      :description "|Strict preprocessing fixture for value equality and hashing on JavaScript host values."
      :init-fn 'type-fail-host-value-equality-strict.main/main!
      :mode :js
      :reload-fn 'type-fail-host-value-equality-strict.main/reload!
      :target :node
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-host-value-equality-strict.main
    %{} 'FileEntry
      :defs $ {}
        'CounterHost $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait CounterHost (:total 'Number)
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object) (:target :node)
          :schema $ :: 'Trait
        'counter-key $ %{} 'CodeEntry
          :doc "|An external-object trait value has no hash, so it cannot be a Map key."
          :code $ quote $ defn counter-key (counter)
            {} $ counter 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'type-fail-host-value-equality-strict.main/CounterHost
        'host-member $ %{} 'CodeEntry
          :doc "|An external-object trait value has no hash, so it cannot be a Set member."
          :code $ quote $ defn host-member (host) (#{} host)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'type-fail-host-value-equality-strict.main/CounterHost
        'identical-host? $ %{} 'CodeEntry
          :doc "|Reference identity remains the explicit host comparison."
          :code $ quote $ defn identical-host? (a b) (identical? a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'type-fail-host-value-equality-strict.main/CounterHost 'type-fail-host-value-equality-strict.main/CounterHost
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                checks $ [] same-host? same-counter? host-member counter-key identical-host?
              , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'same-counter? $ %{} 'CodeEntry
          :doc "|Two external-object trait values compare only by reference."
          :code $ quote $ defn same-counter? (a b) (= a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'type-fail-host-value-equality-strict.main/CounterHost 'type-fail-host-value-equality-strict.main/CounterHost
        'same-host? $ %{} 'CodeEntry (:doc "|Two JsObject values compare only by reference.")
          :code $ quote $ defn same-host? (a b) (= a b)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Bool)
            :args $ [] 'JsObject 'JsObject
      :ns $ %{} 'NsEntry (:doc "|Strict host value equality fixture.")
        :code $ quote $ ns type-fail-host-value-equality-strict.main
