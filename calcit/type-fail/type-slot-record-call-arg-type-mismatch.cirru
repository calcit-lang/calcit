
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-type-slot-record-call-arg) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-type-slot-record-call-arg.main/main!) (:mode :native) (:reload-fn 'type-fail-type-slot-record-call-arg.main/reload!)
      :modules $ []
      :type-slots $ {} (:payload |type-fail-type-slot-record-call-arg.main/User)
  :files $ {}
    |type-fail-type-slot-record-call-arg.main $ %{} :FileEntry
      :defs $ {}
        |User $ %{} :CodeEntry (:doc "|Record type used for bind-type fixture")
          :code $ quote
            defstruct User $ :name 'String
          :examples $ []
          :schema $ :: 'Dynamic
        |main! $ %{} :CodeEntry (:doc "|Entry for type-slot record bind call-site arg type mismatch")
          :code $ quote
            defn main! () (takes-user 1) nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
        |takes-user $ %{} :CodeEntry (:doc "|Schema expects a value matching the bound type slot")
          :code $ quote
            defn takes-user (x) x
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] '*payload
      :ns $ %{} :NsEntry (:doc "|Namespace for type-slot record call-site arg mismatch")
        :code $ quote (ns type-fail-type-slot-record-call-arg.main)
