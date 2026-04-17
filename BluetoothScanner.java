package org.mdmsolutions.scanproj3;

import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothSocket;
import android.os.AsyncTask;
import android.util.Log;

import java.io.IOException;
import java.io.InputStream;
import java.util.Set;
import java.util.UUID;

/**
 * Created by mikmorg on 7/26/15.
 */

public class BluetoothScanner extends AsyncTask<BluetoothDevice, String, Void> {

    public static String SPP_UUID = "00001101-0000-1000-8000-00805F9B34FB";

    private ScannerReader mReader;

    public BluetoothScanner(ScannerReader reader) {
        mReader = reader;
    }

    @Override
    protected Void doInBackground(BluetoothDevice... params)
    {
        BluetoothDevice device = params[0];

        // todo: fix BluetoothManagerCallback
        // 08-01 15:30:50.727    1558-3890/org.mdmsolutions.scanproj3 W/BluetoothAdapter﹕ getBluetoothService() called with no BluetoothManagerCallback

        try
        {
            final BluetoothAdapter btAdapter = BluetoothAdapter.getDefaultAdapter();

            Log.i("ScanProj", "connecting to SPP");
            BluetoothSocket socket = device.createInsecureRfcommSocketToServiceRecord(UUID.fromString(SPP_UUID));
            btAdapter.cancelDiscovery();
            socket.connect();

            Log.i("ScanProj", "starting to read");
            InputStream stream = socket.getInputStream();
            int read = 0;
            byte[] buffer = new byte[256];
            do
            {
                try
                {
                    read = stream.read(buffer);
                    String data = new String(buffer, 0, read);
                    publishProgress(data);
                }
                catch(Exception ex)
                {
                    read = -1;
                }
            }
            while (read > 0 && !isCancelled());

            socket.close();
        }
        catch (IOException e)
        {
            e.printStackTrace();
        }

        return null;
    }

    @Override
    protected void onProgressUpdate(String... values)
    {
        Log.i("ScanProj", "onProgressUpdate started");
        for (String s : values) {
            mReader.readScan(s.trim());
        }

        super.onProgressUpdate(values);
    }

    public static BluetoothDevice getDevice(String deviceName) {
        final BluetoothAdapter btAdapter = BluetoothAdapter.getDefaultAdapter();

        if (btAdapter == null) {
            Log.i("ScanProj", "BT adapter not found.");
            return null;
        }

        Set<BluetoothDevice> pairedDevices = btAdapter.getBondedDevices();
        for (BluetoothDevice device : pairedDevices) {
            Log.i("ScanProj", "device name: " + device.getName() + " " + device.getAddress());
            if (device.getName().equals(deviceName)) {
                return device;
            }
        }

        return null;
    }

}
