defmodule Cache do
  @moduledoc "a tiny in-memory key/value cache backed by a GenServer."
  use GenServer

  def start_link(opts \\ []), do: GenServer.start_link(__MODULE__, %{}, opts)

  def put(server, key, value), do: GenServer.cast(server, {:put, key, value})
  def get(server, key), do: GenServer.call(server, {:get, key})

  @impl true
  def init(state), do: {:ok, state}

  @impl true
  def handle_cast({:put, key, value}, state), do: {:noreply, Map.put(state, key, value)}

  @impl true
  def handle_call({:get, key}, _from, state), do: {:reply, Map.get(state, key), state}
end
