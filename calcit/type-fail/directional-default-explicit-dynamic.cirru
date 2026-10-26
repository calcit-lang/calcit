
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-directional-explicit-dynamic
  :entries $ {} $ :default
    {} (:description "|Directional proof keeps explicit Dynamic storage, passing and wrapping legal.") (:init-fn 'type-fail-directional-explicit-dynamic.main/main!) (:mode :native) (:reload-fn 'type-fail-directional-explicit-dynamic.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-directional-explicit-dynamic.main
    %{} 'FileEntry
      :defs $ {}
        'keep $ %{} 'CodeEntry (:doc "|Dynamic in, Dynamic out: no concrete claim to prove.")
          :code $ quote $ defn keep (x) x
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ [] 'Dynamic
        'main! $ %{} 'CodeEntry (:doc "|Store, pass and wrap Dynamic values, and instantiate a generic with Dynamic.")
          :code $ quote $ defn main! ()
            let
                items $ [] (keep 1) (keep |a)
                wrapped $ {} (:items items)
              println (first items) wrapped
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Dynamic)
            :args $ []
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns type-fail-directional-explicit-dynamic.main
