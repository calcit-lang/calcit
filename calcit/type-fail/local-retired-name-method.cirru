
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-local-retired-name-method
  :entries $ {} $ :default
    {}
      :description "|Fixture for a local struct whose own method shares a retired core alias name."
      :init-fn 'type-fail-local-retired-name-method.main/main!
      :mode :native
      :reload-fn 'type-fail-local-retired-name-method.main/reload!
      :target :node
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-local-retired-name-method.main
    %{} 'FileEntry
      :defs $ {}
        'LocalPath $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct LocalPath (:path String) (:impls Writer)
          :examples $ []
          :schema $ quote StructDef
        'LocalPathImpl $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defimpl LocalPath Writer
            .write-text $ fn (self content) content
          :examples $ []
          :schema $ quote Impl
        'Writer $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait Writer (.write-text AnyFn)
          :examples $ []
          :schema $ quote Trait
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            let
                p $ %{} LocalPath $ :path |/tmp/file
                a $ .write-text p |prefix
                b $ p .write-text |postfix
              , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reload! $ %{} 'CodeEntry (:doc "|Reload handler.")
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry
        :doc "|Fixture for a local struct whose own method shares a retired core alias name."
        :code $ quote $ ns type-fail-local-retired-name-method.main
