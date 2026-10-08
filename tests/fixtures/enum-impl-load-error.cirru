
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |app
  :entries $ {} $ :default
    {} (:description "|impl-traits alias whose trait fails to load") (:init-fn 'app.probe/main!) (:mode :native) (:reload-fn 'app.probe/main!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'app.core $ %{} 'FileEntry
      :defs $ {}
        '%Atom $ %{} 'CodeEntry (:doc |)
          :code $ quote $ def %Atom (impl-traits AtomState HookAtomImpl)
          :examples $ []
        'AtomState $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defenum AtomState (:atom 'Number)
          :examples $ []
          :schema $ :: 'EnumDef
        'HookAtomImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl HookAtomImpl HookAtomOps (.deref deref-atom)
          :examples $ []
          :schema $ :: 'Impl
        'HookAtomOps $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait HookAtomOps
            .deref $ :: 'Fn $ {}
              :args $ [] 'app.core/AtomState
              :return 'Dynamic
          :examples $ []
          :schema $ :: 'Trait
        'deref-atom $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn deref-atom (self) 1
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Number)
            :args $ [] 'app.core/AtomState
        'make-atom $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn make-atom () (%:: %Atom :atom 1)
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'app.core/%Atom)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.core
    'app.probe $ %{} 'FileEntry
      :defs $ {} $ 'main!
        %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            println $ .deref $ make-atom
            , &unit
          :examples $ []
          :schema $ :: 'Fn $ {} (:return 'Unit)
            :args $ []
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns app.probe
          :require $ app.core :refer $ make-atom
