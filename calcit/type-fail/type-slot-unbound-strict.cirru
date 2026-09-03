
{} (:about "|Strict preprocessing fixture for a reachable unbound type slot.") (:package |type-fail-type-slot-unbound-strict) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-type-slot-unbound-strict.main/main!) (:mode :native) (:reload-fn 'type-fail-type-slot-unbound-strict.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |type-fail-type-slot-unbound-strict.main $ %{} 'FileEntry
      :defs $ {}
        |accept-payload $ %{} 'CodeEntry (:doc "|Reachable definition whose payload slot must be bound by the selected entry.")
          :code $ quote
            defn accept-payload (payload) payload
          :examples $ []
          :schema $ :: 'Fn
            {} (:return '*payload)
              :args $ [] '*payload
        |main! $ %{} 'CodeEntry (:doc "|Entry that makes accept-payload reachable.")
          :code $ quote
            defn main! ()
              accept-payload 1
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ []
        |reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote
            defn reload! () &unit
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Unit)
              :args $ []
      :ns $ %{} 'NsEntry (:doc "|Strict unbound type-slot fixture.")
        :code $ quote (ns type-fail-type-slot-unbound-strict.main)
