{}
  :schema-version 1
  :feature 'typed-ffi-capabilities
  :doc "|Wrap native async AnyRef capabilities in nominal Calcit values and expose lifecycle operations as methods."
  :roots $ #{} 'calcit.core/ffi:task 'calcit.core/ffi:response
  :definitions $ {}
    'calcit.core/FfiTask $ {}
      :mode :ensure
      :kind :data
      :doc "|Nominal wrapper for a cancellable native async task capability."
      :schema $ :: 'StructDef
      :code $ quote
        def FfiTask $ impl-traits
          defstruct FfiTask $ :raw 'Dynamic
          , FfiTaskOpsImpl
    'calcit.core/FfiTaskOps $ {}
      :mode :ensure
      :kind :data
      :doc "|Internal method contract for native async task capabilities."
      :schema $ :: 'Trait
      :code $ quote
        deftrait FfiTaskOps
          .cancel $ :: 'Fn
            {}
              :args $ [] 'FfiTask
              :return 'Unit
          .cancel-with $ :: 'Fn
            {}
              :generics $ [] 'T
              :args $ [] 'FfiTask 'T
              :return 'Unit
    'calcit.core/FfiTaskOpsImpl $ {}
      :mode :ensure
      :kind :data
      :doc "|Internal implementation of native async task methods."
      :schema $ :: 'Impl
      :code $ quote
        defimpl FfiTaskOpsImpl FfiTaskOps (.cancel ffi-task:cancel) (.cancel-with ffi-task:cancel-with)
    'calcit.core/ffi:task $ {}
      :mode :ensure
      :kind :fn
      :doc "|Wrap a raw native async task capability at a module adapter boundary."
      :params $ [] 'raw
      :schema $ :: 'Fn
        {}
          :args $ [] 'Dynamic
          :return 'FfiTask
      :code $ quote
        defn ffi:task (raw)
          %{} FfiTask $ :raw raw
    'calcit.core/ffi-task:cancel $ {}
      :mode :ensure
      :kind :fn
      :doc "|Cancel a wrapped native async task with the default reason."
      :params $ [] 'self
      :schema $ :: 'Fn
        {}
          :args $ [] 'FfiTask
          :return 'Unit
      :code $ quote
        defn ffi-task:cancel (self)
          &ffi-task-cancel $ :raw self
    'calcit.core/ffi-task:cancel-with $ {}
      :mode :ensure
      :kind :fn
      :doc "|Cancel a wrapped native async task with an explicit EDN-compatible reason."
      :params $ [] 'self 'reason
      :schema $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'FfiTask 'T
          :return 'Unit
      :code $ quote
        defn ffi-task:cancel-with (self reason)
          &ffi-task-cancel (:raw self) reason
    'calcit.core/FfiResponse $ {}
      :mode :ensure
      :kind :data
      :doc "|Nominal wrapper for an exactly-once native async response capability."
      :schema $ :: 'StructDef
      :code $ quote
        def FfiResponse $ impl-traits
          defstruct FfiResponse $ :raw 'Dynamic
          , FfiResponseOpsImpl
    'calcit.core/FfiResponseOps $ {}
      :mode :ensure
      :kind :data
      :doc "|Internal method contract for native async response capabilities."
      :schema $ :: 'Trait
      :code $ quote
        deftrait FfiResponseOps
          .resolve $ :: 'Fn
            {}
              :generics $ [] 'T
              :args $ [] 'FfiResponse 'T
              :return 'Unit
          .reject $ :: 'Fn
            {}
              :generics $ [] 'T
              :args $ [] 'FfiResponse 'T
              :return 'Unit
    'calcit.core/FfiResponseOpsImpl $ {}
      :mode :ensure
      :kind :data
      :doc "|Internal implementation of native async response methods."
      :schema $ :: 'Impl
      :code $ quote
        defimpl FfiResponseOpsImpl FfiResponseOps (.resolve ffi-response:resolve) (.reject ffi-response:reject)
    'calcit.core/ffi:response $ {}
      :mode :ensure
      :kind :fn
      :doc "|Wrap a raw native async response capability at a module adapter boundary."
      :params $ [] 'raw
      :schema $ :: 'Fn
        {}
          :args $ [] 'Dynamic
          :return 'FfiResponse
      :code $ quote
        defn ffi:response (raw)
          %{} FfiResponse $ :raw raw
    'calcit.core/ffi-response:resolve $ {}
      :mode :ensure
      :kind :fn
      :doc "|Resolve a wrapped native async response exactly once."
      :params $ [] 'self 'value
      :schema $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'FfiResponse 'T
          :return 'Unit
      :code $ quote
        defn ffi-response:resolve (self value)
          &ffi-response-resolve (:raw self) value
    'calcit.core/ffi-response:reject $ {}
      :mode :ensure
      :kind :fn
      :doc "|Reject a wrapped native async response exactly once."
      :params $ [] 'self 'value
      :schema $ :: 'Fn
        {}
          :generics $ [] 'T
          :args $ [] 'FfiResponse 'T
          :return 'Unit
      :code $ quote
        defn ffi-response:reject (self value)
          &ffi-response-reject (:raw self) value
  :edges $ #{}
    :: :type 'calcit.core/ffi:task 'calcit.core/FfiTask
    :: :type 'calcit.core/ffi-task:cancel 'calcit.core/FfiTask
    :: :type 'calcit.core/ffi-task:cancel-with 'calcit.core/FfiTask
    :: :type 'calcit.core/ffi:response 'calcit.core/FfiResponse
    :: :type 'calcit.core/ffi-response:resolve 'calcit.core/FfiResponse
    :: :type 'calcit.core/ffi-response:reject 'calcit.core/FfiResponse
