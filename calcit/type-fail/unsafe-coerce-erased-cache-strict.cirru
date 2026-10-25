
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-unsafe-coerce-erased-cache-strict
  :entries $ {} $ :default
    {}
      :description "|Strict preprocessing fixture for restoring a typed value from a type-erased cache."
      :init-fn 'type-fail-unsafe-coerce-erased-cache-strict.main/main!
      :mode :native
      :reload-fn 'type-fail-unsafe-coerce-erased-cache-strict.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-unsafe-coerce-erased-cache-strict.main
    %{} 'FileEntry
      :defs $ {}
        'erased-call $ %{} 'CodeEntry
          :doc "|Stands in for a cache whose storage erases the value type."
          :code $ quote $ defn erased-call (key f) (f)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic $ :: 'Fn
              {} (:return 'Dynamic)
                :args $ []
        'main! $ %{} 'CodeEntry
          :doc "|Callers use the adapter without needing the feature themselves."
          :code $ quote $ defn main! () (restore-cached :text make-text) &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'make-text $ %{} 'CodeEntry
          :doc "|A typed producer stored through the erased cache."
          :code $ quote $ defn make-text () |fresh
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'String)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
        'restore-cached $ %{} 'CodeEntry
          :doc "|Restores the producer's value from the erased cache; the producer's instantiation supplies the type."
          :code $ quote $ defn restore-cached (key f)
            unsafe-coerce (erased-call key f) 'R
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'R)
            :args $ [] 'K $ :: 'Fn
              {} (:return 'R)
                :args $ []
            :features $ #{} :erased-cache
            :generics $ [] 'K 'R
      :ns $ %{} 'NsEntry (:doc "|Strict erased-cache unsafe-coerce fixture.")
        :code $ quote $ ns type-fail-unsafe-coerce-erased-cache-strict.main
